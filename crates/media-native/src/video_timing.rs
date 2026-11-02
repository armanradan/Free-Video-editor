//! One bounded decoded scan for frame count, presentation timestamps and EOF.
use crate::{CancellationToken, CancellationWatch, ChildGuard, NativeResult};
use media_core::Size;
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Command, Stdio},
};

const MAX_FRAMES: usize = 2_000_000;
const MAX_LINE_BYTES: u64 = 4096;

pub(crate) struct FrameScanSpec<'a> {
    pub time_base: &'a str,
    pub frame_count: Option<u64>,
    pub origin: Option<i64>,
    pub size: Option<Size>,
    /// Only authorized by the caller for ASF WMV-family input.
    pub missing_duration_rate: Option<(u32, u32)>,
}

pub(crate) struct VideoTimeline {
    pub timeline: Vec<i128>,
    pub last_position_us: Option<u64>,
    pub duration_ticks: Option<i64>,
    pub inferred_final_frame_duration_ticks: Option<i64>,
}

/// Constant-space validation in source ticks, before microsecond rounding.
struct DurationGrid {
    period_num: i128,
    period_den: i128,
    first: Option<i128>,
    previous: Option<i128>,
    count: usize,
    matches: bool,
}

impl DurationGrid {
    fn new(rate: (u32, u32), time_num: i128, time_den: i128) -> NativeResult<Self> {
        let (rate_num, rate_den) = rate;
        if rate_num == 0 || rate_den == 0 {
            return Err("WMV terminal-duration fallback requires a positive declared rate".into());
        }
        let period_num = time_den
            .checked_mul(i128::from(rate_den))
            .ok_or("WMV period overflow")?;
        let period_den = time_num
            .checked_mul(i128::from(rate_num))
            .ok_or("WMV period overflow")?;
        Ok(Self {
            period_num,
            period_den,
            first: None,
            previous: None,
            count: 0,
            matches: period_num >= period_den.checked_mul(2).ok_or("WMV period overflow")?,
        })
    }

    fn agrees(&self, ticks: i128, periods: usize) -> NativeResult<bool> {
        let actual = ticks
            .checked_mul(self.period_den)
            .ok_or("WMV grid overflow")?;
        let expected = self
            .period_num
            .checked_mul(i128::try_from(periods)?)
            .ok_or("WMV grid overflow")?;
        Ok(actual
            .checked_sub(expected)
            .ok_or("WMV grid overflow")?
            .unsigned_abs()
            < self.period_den as u128)
    }

    fn observe(&mut self, pts: i128, duration: Option<i128>) -> NativeResult<()> {
        let first = *self.first.get_or_insert(pts);
        self.matches &= self.agrees(
            pts.checked_sub(first).ok_or("WMV grid overflow")?,
            self.count,
        )?;
        if let Some(previous) = self.previous {
            self.matches &=
                self.agrees(pts.checked_sub(previous).ok_or("WMV interval overflow")?, 1)?;
        }
        if let Some(duration) = duration.filter(|duration| *duration > 0) {
            self.matches &= self.agrees(duration, 1)?;
        }
        self.previous = Some(pts);
        self.count += 1;
        Ok(())
    }

    fn infer(&self) -> NativeResult<i64> {
        if self.count < 3 || !self.matches {
            return Err("WMV final duration is missing and the declared rate does not establish an unambiguous constant timestamp grid".into());
        }
        Ok(i64::try_from(
            self.period_num
                .checked_add(self.period_den / 2)
                .ok_or("WMV terminal duration overflow")?
                / self.period_den,
        )?)
    }
}

fn micros(ticks: i128, num: i128, den: i128) -> NativeResult<i128> {
    Ok(ticks
        .checked_mul(num)
        .and_then(|n| n.checked_mul(1_000_000))
        .and_then(|n| n.checked_add(den / 2))
        .ok_or("Frame timestamp overflow")?
        / den)
}

pub(crate) fn scan_video_timestamps(
    path: &Path,
    spec: FrameScanSpec<'_>,
    cancel: &CancellationToken,
) -> NativeResult<VideoTimeline> {
    cancel.check()?;
    let (num, den) = crate::parse_rational(spec.time_base)?;
    if num <= 0 || den <= 0 {
        return Err("Video time base must be positive".into());
    }
    if spec
        .frame_count
        .is_some_and(|count| count == 0 || count > MAX_FRAMES as u64)
    {
        return Err("Video frame count exceeds the supported inspection bound".into());
    }
    let mut command = Command::new("ffprobe");
    command.args(["-v", "error", "-select_streams", "v:0", "-show_frames", "-show_entries",
        "frame=best_effort_timestamp,width,height,duration,interlaced_frame:frame_tags=:frame_side_data=",
        "-of", "compact=p=0:nk=0"]).arg(path).stdout(Stdio::piped());
    let mut child = ChildGuard::spawn(&mut command)?;
    let _watch = CancellationWatch::new(cancel, &[&child]);
    let mut reader = BufReader::new(child.take_stdout()?);
    let mut timeline = Vec::new();
    let mut first_pts = None;
    let mut last = None;
    let mut size = spec.size;
    let mut duration_grid = spec
        .missing_duration_rate
        .map(|rate| DurationGrid::new(rate, num, den))
        .transpose()?;
    loop {
        cancel.check()?;
        let mut bytes = Vec::new();
        let count = reader
            .by_ref()
            .take(MAX_LINE_BYTES + 1)
            .read_until(b'\n', &mut bytes)?;
        if count == 0 {
            break;
        }
        if count as u64 > MAX_LINE_BYTES {
            return Err("Video frame metadata exceeds4096bytes".into());
        }
        let text = std::str::from_utf8(&bytes)?.trim();
        if text.is_empty() {
            continue;
        }
        let mut pts = None;
        let mut width = None;
        let mut height = None;
        let mut duration = None;
        let mut progressive = false;
        for field in text.split('|').filter(|field| !field.is_empty()) {
            let (name, value) = field
                .split_once('=')
                .ok_or("Malformed decoded frame metadata")?;
            match name {
                "best_effort_timestamp" => pts = Some(value.parse::<i128>()?),
                "width" => width = Some(value.parse::<u32>()?),
                "height" => height = Some(value.parse::<u32>()?),
                "duration" if value != "N/A" => duration = Some(value.parse::<i128>()?),
                "duration" => (),
                "interlaced_frame" => progressive = value == "0",
                _ => return Err("Unexpected decoded frame metadata".into()),
            }
        }
        if !progressive {
            return Err("Native input requires progressive decoded video".into());
        }
        if duration.is_some_and(|duration| duration <= 0) {
            return Err("Decoded frame duration must be positive when present".into());
        }
        let pts = pts.ok_or("Missing decoded frame timestamp")?;
        if let Some(grid) = &mut duration_grid {
            grid.observe(pts, duration)?;
        }
        if pts < 0 {
            return Err("Negative decoded video timestamps are unsupported".into());
        }
        let current = Size::new(
            width.ok_or("Missing frame width")?,
            height.ok_or("Missing frame height")?,
        )?;
        if size.is_some_and(|expected| expected != current) {
            return Err(format!(
                "mid-stream video geometry changed at frame {}",
                timeline.len()
            )
            .into());
        }
        size = Some(current);
        let timestamp = micros(pts, num, den)?;
        if timeline
            .last()
            .is_some_and(|previous| timestamp <= *previous)
        {
            return Err("video frame timestamps are not strictly increasing".into());
        }
        if timeline.len() >= MAX_FRAMES {
            return Err("Decoded video exceeds2000000frames".into());
        }
        timeline.push(timestamp);
        if spec
            .frame_count
            .is_some_and(|expected| timeline.len() as u64 > expected)
        {
            return Err("ffprobe decoded more frames than reported".into());
        }
        first_pts.get_or_insert(pts);
        last = Some((pts, duration));
    }
    let status = child.wait_cancellable(cancel)?;
    if !status.success() || !child.diagnostic().trim().is_empty() {
        return Err(child
            .failure("Decoded video timeline inspection", status)
            .into());
    }
    if timeline.is_empty() {
        return Err("Video has no decoded frames".into());
    }
    if let Some(expected) = spec.frame_count
        && timeline.len() as u64 != expected
    {
        return Err(format!(
            "ffprobe decoded {} frames, expected {expected}",
            timeline.len()
        )
        .into());
    }
    let (last_pts, duration) = last.ok_or("Missing final decoded frame")?;
    let inferred_final_frame_duration_ticks = if duration.is_none_or(|duration| duration <= 0) {
        duration_grid
            .as_ref()
            .map(DurationGrid::infer)
            .transpose()?
    } else {
        None
    };
    let duration = duration
        .filter(|duration| *duration > 0)
        .or(inferred_final_frame_duration_ticks.map(i128::from));
    // ASF can advertise a start before its first decoded image. Unknown-count
    // coverage must be anchored to actual decoded presentation, not that header.
    let origin = if spec.frame_count.is_none() {
        first_pts
    } else {
        spec.origin.map(i128::from).or(first_pts)
    }
    .ok_or("Missing video origin")?;
    let duration_ticks = duration
        .filter(|duration| *duration > 0)
        .map(|duration| -> NativeResult<i64> {
            Ok(i64::try_from(
                last_pts
                    .checked_add(duration)
                    .and_then(|end| end.checked_sub(origin))
                    .ok_or("Final video endpoint overflow")?,
            )?)
        })
        .transpose()?;
    if spec.frame_count.is_none() && duration_ticks.is_none() {
        return Err(
            "Unknown-count source requires an explicit final decoded frame duration".into(),
        );
    }
    let last_position_us = Some(u64::try_from(micros(
        last_pts
            .checked_sub(origin)
            .ok_or("Video position overflow")?,
        num,
        den,
    )?)?);
    Ok(VideoTimeline {
        timeline,
        last_position_us,
        duration_ticks,
        inferred_final_frame_duration_ticks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_wmv_missing_duration_requires_verified_declared_grid() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/native-codecs/wmv1-wmav1.wmv");
        let token = CancellationToken::default();
        let spec = || FrameScanSpec {
            time_base: "1/1000",
            frame_count: None,
            origin: Some(1250),
            size: None,
            missing_duration_rate: Some((25, 1)),
        };
        let result = scan_video_timestamps(&path, spec(), &token).unwrap();
        assert_eq!(
            result.timeline,
            (0..30)
                .map(|index| 1_250_000 + index * 40_000)
                .collect::<Vec<_>>()
        );
        assert_eq!(result.inferred_final_frame_duration_ticks, Some(40));
        assert_eq!(result.duration_ticks, Some(1200));
        assert_eq!(result.last_position_us, Some(1160000));
        for rate in [None, Some((50, 1)), Some((0, 1))] {
            assert!(
                scan_video_timestamps(
                    &path,
                    FrameScanSpec {
                        missing_duration_rate: rate,
                        ..spec()
                    },
                    &token
                )
                .is_err()
            );
        }
        // A declared count does not grant permission to estimate an invalid ending.
        assert!(
            scan_video_timestamps(
                &path,
                FrameScanSpec {
                    frame_count: Some(30),
                    missing_duration_rate: Some((30, 1)),
                    ..spec()
                },
                &token
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_grid_checks_fractional_quantization_gaps_drift_and_conflicts() {
        let mut fractional = DurationGrid::new((30000, 1001), 1, 1000).unwrap();
        for index in 0..10000i128 {
            fractional.observe(1250 + index * 1001 / 30, None).unwrap();
        }
        assert_eq!(fractional.infer().unwrap(), 33);
        for timestamps in [
            vec![0, 40],
            vec![0, 40, 120],
            vec![0, 40, 81],
            vec![0, 33, 67, 100],
        ] {
            let mut grid = DurationGrid::new((25, 1), 1, 1000).unwrap();
            for pts in timestamps {
                grid.observe(pts, None).unwrap();
            }
            assert!(grid.infer().is_err());
        }
        let mut conflict = DurationGrid::new((25, 1), 1, 1000).unwrap();
        for pts in [0, 40, 80] {
            conflict.observe(pts, Some(80)).unwrap();
        }
        assert!(conflict.infer().is_err());
        let mut coarse = DurationGrid::new((25, 1), 1, 25).unwrap();
        for pts in [0, 1, 2] {
            coarse.observe(pts, None).unwrap();
        }
        assert!(coarse.infer().is_err());
        assert!(DurationGrid::new((1, u32::MAX), 1, i128::MAX).is_err());
        let mut overflow = DurationGrid::new((25, 1), 1, 1000).unwrap();
        overflow.observe(0, None).unwrap();
        assert!(overflow.observe(i128::MAX, None).is_err());
        // 2.5 source ticks per period: positive half ties round upward.
        let mut ties = DurationGrid::new((2, 1), 1, 5).unwrap();
        for pts in [0, 2, 5, 7] {
            ties.observe(pts, None).unwrap();
        }
        assert_eq!(ties.infer().unwrap(), 3);
    }

    #[test]
    fn actual_missing_count_uses_decoded_frames_and_last_duration() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/native-mpeg1.mpg");
        let token = CancellationToken::default();
        let spec = || FrameScanSpec {
            time_base: "1/90000",
            frame_count: None,
            origin: Some(112500),
            size: None,
            missing_duration_rate: None,
        };
        let result = scan_video_timestamps(&path, spec(), &token).unwrap();
        assert_eq!(result.timeline.len(), 50);
        assert_eq!(result.duration_ticks, Some(180000));
        assert_eq!(result.inferred_final_frame_duration_ticks, None);
        assert_eq!(result.last_position_us, Some(1960000));
        assert!(
            result
                .timeline
                .iter()
                .enumerate()
                .all(|(i, pts)| *pts == 1_250_000 + i as i128 * 40_000)
        );
        for count in [49, 51] {
            assert!(
                scan_video_timestamps(
                    &path,
                    FrameScanSpec {
                        frame_count: Some(count),
                        ..spec()
                    },
                    &token
                )
                .is_err()
            );
        }
        assert!(micros(i128::MAX, 90000, 1).is_err());
        token.cancel();
        assert!(scan_video_timestamps(&path, spec(), &token).is_err());
    }

    #[test]
    fn unknown_count_retains_real_vfr_and_rejects_broken_container() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let token = CancellationToken::default();
        let spec = || FrameScanSpec {
            time_base: "1/15360",
            frame_count: None,
            origin: Some(19200),
            size: None,
            missing_duration_rate: None,
        };
        let result =
            scan_video_timestamps(&root.join("m35-vfr-offset.mp4"), spec(), &token).unwrap();
        assert_eq!(result.timeline.len(), 36);
        let intervals = result
            .timeline
            .windows(2)
            .map(|p| p[1] - p[0])
            .collect::<Vec<_>>();
        assert!(intervals.iter().min() < intervals.iter().max());
        assert!(result.duration_ticks.is_some_and(|d| d > 0));
        let broken =
            std::env::temp_dir().join(format!("diaxus-broken-video-{}.mp4", std::process::id()));
        let bytes = std::fs::read(root.join("m35-vfr-offset.mp4")).unwrap();
        std::fs::write(&broken, &bytes[..32]).unwrap();
        assert!(scan_video_timestamps(&broken, spec(), &token).is_err());
        std::fs::remove_file(broken).unwrap();
    }
}
