#include "sdk_support.h"

int wmain(int argc, wchar_t** argv) {
    try {
        const bool missing_input_test = argc == 6 && std::wstring(argv[5]) == L"--missing-input-test";
        if (argc != 5 && !missing_input_test)
            throw std::runtime_error("usage: probe <sdk-root> <dxgi-index> <exact-adapter-name> <new-output-directory> [--missing-input-test]");
        const fs::path sdk = fs::canonical(argv[1]);
        if (!fs::is_regular_file(sdk / "bin/Windows/x64/rel/nvngx_vsr.dll"))
            throw std::runtime_error("missing SDK runtime: bin/Windows/x64/rel/nvngx_vsr.dll; no alternate SDK path");
        size_t consumed = 0;
        const unsigned long ordinal = std::stoul(argv[2], &consumed);
        if (consumed != std::wcslen(argv[2]) || ordinal > 15) throw std::runtime_error("invalid DXGI index");
        const fs::path output = fs::absolute(argv[4]);
        if (!fs::create_directory(output)) throw std::runtime_error("output must be a new directory");
        const fs::path cache = output / "ngx-cache";
        fs::create_directory(cache);
        ComPtr<IDXGIFactory1> factory;
        hr_check("create-dxgi-factory", CreateDXGIFactory1(IID_PPV_ARGS(&factory)));
        ComPtr<IDXGIAdapter1> adapter;
        hr_check("selected-dxgi-adapter", factory->EnumAdapters1(UINT(ordinal), &adapter));
        DXGI_ADAPTER_DESC1 identity{};
        hr_check("adapter-description", adapter->GetDesc1(&identity));
        std::cout << "{\"stage\":\"adapter\",\"dxgi_index\":" << ordinal
                  << ",\"name\":" << json(utf8(identity.Description))
                  << ",\"vendor\":" << identity.VendorId << ",\"device\":" << identity.DeviceId
                  << ",\"luid_high\":" << identity.AdapterLuid.HighPart
                  << ",\"luid_low\":" << identity.AdapterLuid.LowPart << "}" << std::endl;
        if (identity.VendorId != 0x10de || std::wstring(identity.Description) != argv[3])
            throw std::runtime_error("explicit NVIDIA adapter identity mismatch; no fallback");
        ComPtr<ID3D11Device> device;
        ComPtr<ID3D11DeviceContext> context;
        D3D_FEATURE_LEVEL level{};
        const D3D_FEATURE_LEVEL requested[] = {D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0};
        hr_check("create-d3d11-device", D3D11CreateDevice(adapter.Get(), D3D_DRIVER_TYPE_UNKNOWN, nullptr,
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, requested, 2, D3D11_SDK_VERSION, &device, &level, &context));
        SdkState state;
        state.device = device.Get();
        const std::wstring feature_path = (sdk / "bin/Windows/x64/rel").wstring();
        const wchar_t* paths[] = {feature_path.c_str()};
        NVSDK_NGX_FeatureCommonInfo common{};
        common.PathListInfo.Path = paths;
        common.PathListInfo.Length = 1;
        ngx_check("initialize", NVSDK_NGX_D3D11_Init(0, cache.c_str(), device.Get(), &common));
        state.initialized = true;
        ngx_check("capabilities", NVSDK_NGX_D3D11_GetCapabilityParameters(&state.parameters));
        int available = 0, update = -1, feature_result = 0;
        ngx_check("vsr-available-query", NVSDK_NGX_Parameter_GetI(state.parameters, NVSDK_NGX_Parameter_VSR_Available, &available));
        const auto update_result = NVSDK_NGX_Parameter_GetI(state.parameters, NVSDK_NGX_Parameter_VSR_NeedsUpdatedDriver, &update);
        const auto init_result = NVSDK_NGX_Parameter_GetI(state.parameters, NVSDK_NGX_Parameter_VSR_FeatureInitResult, &feature_result);
        std::cout << "{\"stage\":\"vsr-capability\",\"available\":" << available
                  << ",\"needs_update\":" << update << ",\"update_query\":" << json(hex_code(uint32_t(update_result)))
                  << ",\"feature_init_query\":" << json(hex_code(uint32_t(init_result)))
                  << ",\"feature_init_result\":" << json(hex_code(uint32_t(feature_result))) << "}" << std::endl;
        if (!available) throw std::runtime_error("VSR capability is unavailable; inspect exact SDK statuses");
        size_t scratch = 0;
        ngx_check("scratch-size", NVSDK_NGX_D3D11_GetScratchBufferSize(NVSDK_NGX_Feature_VSR, state.parameters, &scratch));
        if (scratch) throw std::runtime_error("nonzero scratch request requires an explicit implementation");
        NVSDK_NGX_Feature_Create_Params create{};
        ngx_check("create-vsr", NGX_D3D11_CREATE_VSR_EXT(context.Get(), &state.feature, state.parameters, &create));
        if (missing_input_test) {
            NVSDK_NGX_D3D11_VSR_Eval_Params invalid{};
            invalid.InputSubrectSize = {192, 108};
            invalid.OutputSubrectSize = {384, 216};
            invalid.QualityLevel = NVSDK_NGX_VSR_Quality_High;
            ngx_check("evaluate-missing-resources", NGX_D3D11_EVALUATE_VSR_EXT(
                context.Get(), state.feature, state.parameters, &invalid));
            throw std::runtime_error("SDK accepted missing resources; rejection test unproven");
        }
        constexpr UINT width = 192, height = 108;
        std::vector<uint8_t> last_enhanced;
        unsigned cases = 0;
        for (UINT scale : {2u, 4u}) for (unsigned pattern : {0u, 1u}) {
            std::vector<uint8_t> input(size_t(width) * height * 4);
            for (UINT y = 0; y < height; ++y) for (UINT x = 0; x < width; ++x) {
                const size_t i = (size_t(y) * width + x) * 4;
                input[i] = uint8_t((x * (pattern ? 7 : 3) + y * 5) % 256);
                input[i + 1] = uint8_t(((x / 5 + y / 7 + pattern) % 2) * 180 + 30);
                input[i + 2] = uint8_t((y * 9 + x * 2 + pattern * 103) % 256);
                input[i + 3] = 255;
            }
            D3D11_TEXTURE2D_DESC desc{};
            desc.Width = width; desc.Height = height; desc.MipLevels = 1; desc.ArraySize = 1;
            desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM; desc.SampleDesc.Count = 1;
            desc.Usage = D3D11_USAGE_DEFAULT; desc.BindFlags = D3D11_BIND_SHADER_RESOURCE;
            D3D11_SUBRESOURCE_DATA initial{input.data(), width * 4, 0};
            ComPtr<ID3D11Texture2D> source, target;
            hr_check("create-input", device->CreateTexture2D(&desc, &initial, &source));
            if (readback(device.Get(), context.Get(), source.Get()) != input)
                throw std::runtime_error("input staging is not exact RGBA8");
            desc.Width *= scale; desc.Height *= scale;
            desc.BindFlags |= D3D11_BIND_UNORDERED_ACCESS;
            hr_check("create-output", device->CreateTexture2D(&desc, nullptr, &target));
            ComPtr<ID3D11UnorderedAccessView> target_view;
            hr_check("create-output-view", device->CreateUnorderedAccessView(target.Get(), nullptr, &target_view));
            std::vector<uint8_t> baseline;
            for (auto quality : {NVSDK_NGX_VSR_Quality_Bicubic, NVSDK_NGX_VSR_Quality_High}) {
                // Give both quality modes identical defined destination contents;
                // no allocator leftovers from a prior case may enter comparison.
                const float clear[4] = {0.f, 0.f, 0.f, 1.f};
                context->ClearUnorderedAccessViewFloat(target_view.Get(), clear);
                NVSDK_NGX_D3D11_VSR_Eval_Params eval{};
                eval.pInput = source.Get(); eval.pOutput = target.Get();
                eval.InputSubrectSize = {width, height};
                eval.OutputSubrectSize = {width * scale, height * scale};
                eval.QualityLevel = quality;
                ngx_check("evaluate", NGX_D3D11_EVALUATE_VSR_EXT(context.Get(), state.feature, state.parameters, &eval));
                auto pixels = readback(device.Get(), context.Get(), target.Get());
                uint8_t maximum = 0; uint64_t rgb_sum = 0;
                for (size_t i = 0; i < pixels.size(); ++i) if (i % 4 != 3) {
                    maximum = std::max(maximum, pixels[i]); rgb_sum += pixels[i];
                }
                if (!maximum || !rgb_sum) throw std::runtime_error("VSR returned black output");
                const std::string name = std::to_string(scale) + "x-pattern" + std::to_string(pattern)
                    + "-quality" + std::to_string(unsigned(quality)) + ".rgba";
                write_raw(output / name, pixels);
                std::cout << "{\"stage\":\"output\",\"scale\":" << scale << ",\"pattern\":" << pattern
                          << ",\"quality\":" << unsigned(quality) << ",\"width\":" << desc.Width
                          << ",\"height\":" << desc.Height << ",\"bytes\":" << pixels.size()
                          << ",\"rgb_sum\":" << rgb_sum << ",\"file\":" << json(name) << "}" << std::endl;
                if (quality == NVSDK_NGX_VSR_Quality_Bicubic) baseline = pixels;
                else {
                    if (pixels == baseline) throw std::runtime_error("high quality equals SDK bicubic; enhancement unproven");
                    if (pattern && pixels == last_enhanced) throw std::runtime_error("distinct inputs returned identical output");
                    last_enhanced = std::move(pixels);
                }
            }
            ++cases;
        }
        record_runtime_modules();
        context->ClearState(); context->Flush();
        if (!state.close()) throw std::runtime_error("SDK cleanup returned a failure");
        std::cout << "{\"status\":\"pilot-passed-not-product-accepted\",\"cases\":" << cases
                  << ",\"input_uploads\":4,\"diagnostic_readbacks\":12,\"format\":\"opaque-RGBA8-SDR\"}" << std::endl;
        return 0;
    } catch (const std::exception& error) {
        std::cout << "{\"status\":\"failed\",\"error\":" << json(error.what()) << "}" << std::endl;
        return 1;
    }
}
