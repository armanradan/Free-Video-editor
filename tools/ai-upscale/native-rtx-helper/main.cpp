// Optional source-only Windows helper. SDK runtime remains separately installed.
#include "../native-rtx-probe/sdk_support.h"
#include <array>
#include <dxgi1_4.h>
#include <cstdio>
#include <cstdlib>
#include <fcntl.h>
#include <io.h>
#ifdef RTX_SAMPLE_REFERENCE
#include <rtx_video_api.h>
#endif

// Explicit little-endian u32 fields; never serialize C++ structs over the pipe.
// magic,version,op,request-id,generation,width,height,scale,color,byte-count.
using Header = std::array<uint32_t, 10>;
constexpr uint32_t request_magic = 0x31585452, response_magic = 0x31595452;
// BT.709 primaries, sRGB transfer, full-range encoded RGB; no HDR conversion.
constexpr uint32_t color_sdr709_rgba8 = 1;
static void diagnostic_barrier(const char* phase, uint64_t frame) {
#ifdef RTX_DIAGNOSTIC
    // Deliberate failures belong only to a separately compiled test executable.
    char configured[64]{}; size_t count = 0;
    getenv_s(&count, configured, sizeof(configured), "DIAXUS_RTX_TEST_PHASE");
    if (!count || std::strcmp(configured, phase)) return;
    char action[32]{};
    getenv_s(&count, action, sizeof(action), "DIAXUS_RTX_TEST_ACTION");
    // Let the first frame complete so failures also exercise partial export cleanup.
    if (frame != 1) return;
    std::cerr << "{\"stage\":\"test-barrier\",\"phase\":" << json(phase)
              << ",\"action\":" << json(action) << ",\"frame\":" << frame << "}" << std::endl;
    if (!std::strcmp(action, "crash")) ExitProcess(73);
    if (!std::strcmp(action, "stall")) Sleep(INFINITE);
    throw std::runtime_error("invalid diagnostic action");
#else
    (void)phase; (void)frame;
#endif
}
static void send(const Header& h, const std::vector<uint8_t>& bytes = {}) {
    for (uint32_t value : h) for (unsigned shift = 0; shift < 32; shift += 8)
        if (std::fputc(int((value >> shift) & 255), stdout) == EOF)
            throw std::runtime_error("protocol: broken output pipe");
    if ((!bytes.empty() && std::fwrite(bytes.data(), 1, bytes.size(), stdout) != bytes.size())
        || std::fflush(stdout)) throw std::runtime_error("protocol: output write failed");
}
static bool receive(Header& h) {
    uint8_t data[40];
    const size_t n = std::fread(data, 1, sizeof(data), stdin);
    if (!n && std::feof(stdin)) return false;
    if (n != sizeof(data)) throw std::runtime_error("protocol: truncated header");
    for (size_t i = 0; i < h.size(); ++i)
        h[i] = uint32_t(data[i*4]) | (uint32_t(data[i*4+1]) << 8)
             | (uint32_t(data[i*4+2]) << 16) | (uint32_t(data[i*4+3]) << 24);
    return true;
}

int wmain(int argc, wchar_t** argv) {
    // SDK audit/status records go only to stderr; stdout is binary protocol.
    std::cout.rdbuf(std::cerr.rdbuf());
    _setmode(_fileno(stdin), _O_BINARY); _setmode(_fileno(stdout), _O_BINARY);
    try {
        if (argc != 5) throw std::runtime_error("usage: helper <sdk-root> <dxgi-index> <exact-adapter-name> <new-cache-directory>");
        const auto sdk = fs::canonical(argv[1]);
        const auto runtime = sdk / "bin/Windows/x64/rel";
        if (!fs::is_regular_file(runtime / "nvngx_vsr.dll"))
            throw std::runtime_error("missing SDK runtime: nvngx_vsr.dll");
        size_t consumed = 0;
        const auto ordinal = std::stoul(argv[2], &consumed);
        if (consumed != std::wcslen(argv[2]) || ordinal > 15) throw std::runtime_error("invalid DXGI index");
        const auto cache = fs::absolute(argv[4]);
        if (!fs::create_directory(cache)) throw std::runtime_error("cache must be a new directory");
        ComPtr<IDXGIFactory1> factory;
        hr_check("create-factory", CreateDXGIFactory1(IID_PPV_ARGS(&factory)));
        ComPtr<IDXGIAdapter1> adapter;
        hr_check("selected-adapter", factory->EnumAdapters1(UINT(ordinal), &adapter));
        DXGI_ADAPTER_DESC1 identity{};
        hr_check("adapter-description", adapter->GetDesc1(&identity));
        if (identity.VendorId != 0x10de || std::wstring(identity.Description) != argv[3])
            throw std::runtime_error("explicit NVIDIA adapter identity mismatch; no fallback");
        ComPtr<ID3D11Device> device;
        ComPtr<ID3D11DeviceContext> context;
        D3D_FEATURE_LEVEL level{};
        const D3D_FEATURE_LEVEL levels[] = {D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0};
        hr_check("create-device", D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, levels, 2, D3D11_SDK_VERSION, &device, &level, &context));
#ifdef RTX_SAMPLE_REFERENCE
        // Test executable links the installed, unmodified SDK sample source.
        struct SampleState { ~SampleState() { rtx_video_api_dx11_shutdown(); } } state;
        if (!rtx_video_api_dx11_create(device.Get(), API_BOOL_FAIL, API_BOOL_SUCCESS))
            throw std::runtime_error("SDK sample create returned API_BOOL_FAIL");
#else
        SdkState state;
        state.device = device.Get();
        const std::wstring path = runtime.wstring();
        const wchar_t* paths[] = {path.c_str()};
        NVSDK_NGX_FeatureCommonInfo common{};
        common.PathListInfo.Path = paths; common.PathListInfo.Length = 1;
        ngx_check("initialize", NVSDK_NGX_D3D11_Init(0, cache.c_str(), device.Get(), &common));
        state.initialized = true;
        ngx_check("capabilities", NVSDK_NGX_D3D11_GetCapabilityParameters(&state.parameters));
        int available = 0;
        ngx_check("vsr-available", NVSDK_NGX_Parameter_GetI(state.parameters, NVSDK_NGX_Parameter_VSR_Available, &available));
        if (!available) throw std::runtime_error("SDK VSR unavailable on selected device");
        size_t scratch = 0;
        ngx_check("scratch-size", NVSDK_NGX_D3D11_GetScratchBufferSize(NVSDK_NGX_Feature_VSR, state.parameters, &scratch));
        if (scratch) throw std::runtime_error("unsupported nonzero SDK scratch request");
        NVSDK_NGX_Feature_Create_Params create{};
        ngx_check("create-vsr", NGX_D3D11_CREATE_VSR_EXT(context.Get(), &state.feature, state.parameters, &create));
#endif
        record_runtime_modules();
        ComPtr<IDXGIAdapter3> budget_adapter;
        hr_check("query-memory-budget-interface", adapter.As(&budget_adapter));
        uint64_t sampled_local_peak = 0;
        const auto admit = [&] {
            DXGI_QUERY_VIDEO_MEMORY_INFO memory{};
            hr_check("query-local-memory-budget", budget_adapter->QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &memory));
            sampled_local_peak = std::max(sampled_local_peak, memory.CurrentUsage);
            constexpr uint64_t reserve = 256ull*1024*1024, process_limit = 1536ull*1024*1024;
            if (memory.CurrentUsage > process_limit || memory.Budget < memory.CurrentUsage
                || memory.Budget - memory.CurrentUsage < reserve)
                throw std::runtime_error("resource: sampled DXGI local-memory budget exhausted; no fallback");
        };
        admit();
        std::ostringstream ready;
        ready << "{\"stage\":\"ready\",\"protocol\":1,\"sdk\":\"1.1.0\",\"pid\":" << GetCurrentProcessId() << ",\"dxgi_index\":" << ordinal
            << ",\"adapter_name\":" << json(utf8(identity.Description))
            << ",\"vendor_id\":" << identity.VendorId << ",\"device_id\":" << identity.DeviceId
            << ",\"luid_high\":" << identity.AdapterLuid.HighPart << ",\"luid_low\":" << identity.AdapterLuid.LowPart
            << ",\"color\":\"opaque-rgba8-full-range-bt709-srgb\",\"quality\":3,\"max_width\":192,\"max_height\":108,\"in_flight\":1}";
        const auto ready_text = ready.str();
        if (ready_text.size() > 8192) throw std::runtime_error("oversized helper identity");
        std::cerr << ready_text << std::endl;
        send({response_magic, 1, 0, 0, 0, 192, 108, 0, color_sdr709_rgba8, uint32_t(ready_text.size())},
            std::vector<uint8_t>(ready_text.begin(), ready_text.end()));
        Header h{};
        uint32_t last_id = 0, generation = 0;
        uint64_t frames = 0, uploads = 0, readbacks = 0;
        while (receive(h)) {
            if (h[0] != request_magic || h[1] != 1) throw std::runtime_error("protocol: unsupported magic/version");
            if (h[2] == 2) {
                if (h[3] || h[4] || h[5] || h[6] || h[7] || h[8] || h[9])
                    throw std::runtime_error("protocol: invalid shutdown header");
                break;
            }
            if (h[2] != 1 || !h[3] || h[3] <= last_id || !h[4] || (generation && generation != h[4]))
                throw std::runtime_error("protocol: invalid operation/id/generation");
            if (!h[5] || h[5] > 192 || !h[6] || h[6] > 108 || (h[7] != 2 && h[7] != 4)
                || h[8] != color_sdr709_rgba8 || uint64_t(h[5])*h[6]*4 != h[9])
                throw std::runtime_error("protocol: unsupported geometry/scale/color/length");
            // Validate all sizes before allocation/read; at most 82,944 input bytes.
            diagnostic_barrier("before-payload", frames);
            std::vector<uint8_t> input(h[9]);
            if (std::fread(input.data(), 1, input.size(), stdin) != input.size())
                throw std::runtime_error("protocol: truncated frame payload");
            for (size_t i = 3; i < input.size(); i += 4)
                if (input[i] != 255) throw std::runtime_error("protocol: nonopaque input unsupported");
            admit();
            D3D11_TEXTURE2D_DESC desc{};
            desc.Width = h[5]; desc.Height = h[6]; desc.MipLevels = 1; desc.ArraySize = 1;
            desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM; desc.SampleDesc.Count = 1;
            desc.Usage = D3D11_USAGE_DEFAULT; desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
            D3D11_SUBRESOURCE_DATA data{input.data(), h[5]*4, 0};
            ComPtr<ID3D11Texture2D> source, target;
            hr_check("create-input", device->CreateTexture2D(&desc, &data, &source));
            desc.Width *= h[7]; desc.Height *= h[7]; desc.BindFlags |= D3D11_BIND_UNORDERED_ACCESS;
            hr_check("create-output", device->CreateTexture2D(&desc, nullptr, &target));
            ComPtr<ID3D11UnorderedAccessView> view;
            hr_check("create-output-view", device->CreateUnorderedAccessView(target.Get(), nullptr, &view));
            const float clear[] = {0.f, 0.f, 0.f, 1.f};
            context->ClearUnorderedAccessViewFloat(view.Get(), clear);
#ifdef RTX_SAMPLE_REFERENCE
            API_VSR_Setting setting{3};
            API_RECT in_rect{0, 0, h[5], h[6]}, out_rect{0, 0, desc.Width, desc.Height};
            if (!rtx_video_api_dx11_evaluate(source.Get(), target.Get(), in_rect, out_rect, &setting, nullptr))
                throw std::runtime_error("SDK sample evaluate returned API_BOOL_FAIL");
#else
            NVSDK_NGX_D3D11_VSR_Eval_Params eval{};
            eval.pInput = source.Get(); eval.pOutput = target.Get();
            eval.InputSubrectSize = {h[5], h[6]}; eval.OutputSubrectSize = {desc.Width, desc.Height};
            eval.QualityLevel = NVSDK_NGX_VSR_Quality_High;
            ngx_check("evaluate", NGX_D3D11_EVALUATE_VSR_EXT(context.Get(), state.feature, state.parameters, &eval));
#endif
            auto pixels = readback(device.Get(), context.Get(), target.Get());
            admit();
            for (size_t i = 3; i < pixels.size(); i += 4)
                if (pixels[i] != 255) throw std::runtime_error("SDK returned nonopaque output");
            diagnostic_barrier("before-response", frames);
            send({response_magic, 1, 1, h[3], h[4], desc.Width, desc.Height, h[7], h[8], uint32_t(pixels.size())}, pixels);
            last_id = h[3]; generation = h[4]; ++frames; uploads += input.size(); readbacks += pixels.size();
        }
        context->ClearState(); context->Flush();
#ifndef RTX_SAMPLE_REFERENCE
        if (!state.close()) throw std::runtime_error("SDK cleanup failed");
#else
        rtx_video_api_dx11_shutdown();
#endif
        std::cerr << "{\"stage\":\"closed\",\"frames\":" << frames << ",\"host_to_gpu_bytes\":" << uploads
            << ",\"gpu_to_host_bytes\":" << readbacks << ",\"sampled_local_peak\":" << sampled_local_peak << "}" << std::endl;
        send({response_magic, 1, 2, 0, 0, 0, 0, 0, 0, 0});
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "{\"stage\":\"failed\",\"error\":" << json(e.what()) << "}" << std::endl;
        return 1;
    }
}
