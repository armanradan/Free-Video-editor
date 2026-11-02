// Copyright Diaxus contributors. Licensed under the repository license.
// Private little-endian protocol v1: magic[8], PCM u32 rate/channels/bits/align;
// records: i64 PTS and duration (100ns), u32 byte length/flags, then PCM bytes.
// flags: 0=data, 1=EOF, 2=stream tick. No diagnostic text on stdout.
#include <windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#include <wrl/client.h>
#include <cstdint>
#include <cstdio>
#include <stdexcept>
#include <string>
using Microsoft::WRL::ComPtr;
static constexpr DWORD audio = static_cast<DWORD>(MF_SOURCE_READER_FIRST_AUDIO_STREAM);
static void need(HRESULT hr, const char* step) {
    if (FAILED(hr)) throw std::runtime_error(std::string(step) + " HRESULT=" + std::to_string(static_cast<unsigned long>(hr)));
}
static void write(const void* bytes, DWORD size) {
    DWORD sent = 0;
    if (!WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), bytes, size, &sent, nullptr) || sent != size)
        throw std::runtime_error("WMA protocol write failed");
}
template<class T> static void number(T value) { write(&value, sizeof(value)); }
static void record(LONGLONG pts, LONGLONG duration, DWORD size, DWORD flag) {
    number(pts); number(duration); number(size); number(flag);
}
int wmain(int argc, wchar_t** argv) {
    if (argc != 2) return 2;
    bool com = false, mf = false;
    int result = 0;
    try {
        need(CoInitializeEx(nullptr, COINIT_MULTITHREADED), "COM"); com = true;
        need(MFStartup(MF_VERSION), "Media Foundation startup"); mf = true;
        ComPtr<IMFSourceReader> reader;
        need(MFCreateSourceReaderFromURL(argv[1], nullptr, &reader), "Open WMA source");
        need(reader->SetStreamSelection(static_cast<DWORD>(MF_SOURCE_READER_ALL_STREAMS), FALSE), "Deselect streams");
        need(reader->SetStreamSelection(audio, TRUE), "Select audio");
        ComPtr<IMFMediaType> native, requested, actual;
        need(reader->GetNativeMediaType(audio, 0, &native), "Native audio type");
        GUID subtype{};
        need(native->GetGUID(MF_MT_SUBTYPE, &subtype), "Native audio codec");
        GUID wma1 = MFAudioFormat_WMAudioV8; wma1.Data1 = 0x0160;
        if (subtype != wma1 && subtype != MFAudioFormat_WMAudioV8 && subtype != MFAudioFormat_WMAudioV9)
            throw std::runtime_error("Only WMA1/2 and WMA Pro are supported");
        need(MFCreateMediaType(&requested), "Create PCM type");
        need(requested->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Audio), "Audio type");
        need(requested->SetGUID(MF_MT_SUBTYPE, MFAudioFormat_PCM), "PCM type");
        need(reader->SetCurrentMediaType(audio, nullptr, requested.Get()), "Select Windows WMA decoder");
        need(reader->GetCurrentMediaType(audio, &actual), "Decoded type");
        UINT32 rate = 0, channels = 0, bits = 0, align = 0;
        need(actual->GetUINT32(MF_MT_AUDIO_SAMPLES_PER_SECOND, &rate), "Sample rate");
        need(actual->GetUINT32(MF_MT_AUDIO_NUM_CHANNELS, &channels), "Channels");
        need(actual->GetUINT32(MF_MT_AUDIO_BITS_PER_SAMPLE, &bits), "Bits");
        need(actual->GetUINT32(MF_MT_AUDIO_BLOCK_ALIGNMENT, &align), "Alignment");
        if (rate < 8000 || rate > 192000 || !channels || channels > 2 ||
            (bits != 16 && bits != 24 && bits != 32) || align != channels * (bits / 8))
            throw std::runtime_error("WMA PCM envelope: 8-192kHz, mono/stereo, integer16/24/32");
        write("DXWMA001", 8); number(rate); number(channels); number(bits); number(align);
        for (unsigned count = 0; ; ++count) {
            if (count >= 2000000) throw std::runtime_error("WMA record limit exceeded");
            DWORD stream = 0, flags = 0;
            LONGLONG timestamp = 0;
            ComPtr<IMFSample> sample;
            need(reader->ReadSample(audio, 0, &stream, &flags, &timestamp, &sample), "Decode WMA audio");
            if (flags & (MF_SOURCE_READERF_ERROR | MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED | MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED | MF_SOURCE_READERF_NEWSTREAM))
                throw std::runtime_error("WMA decoder stream/format changed or failed");
            if (sample) {
                LONGLONG duration = 0;
                need(sample->GetSampleTime(&timestamp), "Sample timestamp");
                need(sample->GetSampleDuration(&duration), "Sample duration");
                ComPtr<IMFMediaBuffer> buffer;
                need(sample->ConvertToContiguousBuffer(&buffer), "PCM buffer");
                DWORD length = 0;
                need(buffer->GetCurrentLength(&length), "PCM length");
                if (!length || length > 1048576 || length % align || duration <= 0)
                    throw std::runtime_error("Invalid or oversized WMA PCM block");
                BYTE* bytes = nullptr;
                need(buffer->Lock(&bytes, nullptr, nullptr), "Lock PCM");
                try { record(timestamp, duration, length, 0); write(bytes, length); }
                catch (...) { buffer->Unlock(); throw; }
                need(buffer->Unlock(), "Unlock PCM");
            } else if (flags & MF_SOURCE_READERF_STREAMTICK) {
                record(timestamp, 0, 0, 2);
            } else if (!(flags & MF_SOURCE_READERF_ENDOFSTREAM)) {
                throw std::runtime_error("WMA decoder returned no sample or stream tick");
            }
            if (flags & MF_SOURCE_READERF_ENDOFSTREAM) { record(0, 0, 0, 1); break; }
        }
    } catch (const std::exception& error) { std::fprintf(stderr, "%s\n", error.what()); result = 1; }
    if (mf) MFShutdown();
    if (com) CoUninitialize();
    return result;
}
