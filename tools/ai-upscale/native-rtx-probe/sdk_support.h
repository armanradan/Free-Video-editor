#pragma once
// Local, source-only RTX Video SDK feasibility probe. No SDK code/binaries vendored.
// Uses the separately licensed installed headers/library and explicit D3D11 staging.
#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <d3d11.h>
#include <dxgi1_2.h>
#include <tlhelp32.h>
#include <wrl/client.h>
#include <nvsdk_ngx.h>
#include <nvsdk_ngx_helpers_vsr.h>
#include <algorithm>
#include <cstdint>
#include <cwchar>
#include <cwctype>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

using Microsoft::WRL::ComPtr;
namespace fs = std::filesystem;

static std::string hex_code(uint32_t value) {
    std::ostringstream s;
    s << "0x" << std::hex << std::setw(8) << std::setfill('0') << value;
    return s.str();
}
static std::string json(const std::string& value) {
    std::ostringstream s;
    s << '"';
    for (unsigned char c : value) {
        if (c == '\\' || c == '"') s << '\\' << c;
        else if (c < 32) s << "\\u" << std::hex << std::setw(4) << std::setfill('0') << unsigned(c);
        else s << c;
    }
    s << '"';
    return s.str();
}
static std::string utf8(const wchar_t* value) {
    const int n = WideCharToMultiByte(CP_UTF8, 0, value, -1, nullptr, 0, nullptr, nullptr);
    if (n <= 0) throw std::runtime_error("UTF-8 conversion failed");
    std::string result(size_t(n), '\0');
    WideCharToMultiByte(CP_UTF8, 0, value, -1, result.data(), n, nullptr, nullptr);
    result.pop_back();
    return result;
}
static void hr_check(const char* stage, HRESULT result) {
    if (FAILED(result)) throw std::runtime_error(std::string(stage) + ": " + hex_code(uint32_t(result)));
}
static void ngx_check(const char* stage, NVSDK_NGX_Result result) {
    const char* meaning = "other-sdk-status";
    switch (result) {
        case NVSDK_NGX_Result_Success: meaning = "success"; break;
        case NVSDK_NGX_Result_FAIL_FeatureNotSupported: meaning = "feature-not-supported"; break;
        case NVSDK_NGX_Result_FAIL_PlatformError: meaning = "platform-error"; break;
        case NVSDK_NGX_Result_FAIL_InvalidParameter: meaning = "invalid-parameter"; break;
        case NVSDK_NGX_Result_FAIL_NotInitialized: meaning = "not-initialized"; break;
        case NVSDK_NGX_Result_FAIL_UnsupportedInputFormat: meaning = "unsupported-input-format"; break;
        case NVSDK_NGX_Result_FAIL_OutOfDate: meaning = "out-of-date"; break;
        case NVSDK_NGX_Result_FAIL_OutOfGPUMemory: meaning = "out-of-gpu-memory"; break;
        case NVSDK_NGX_Result_FAIL_UnsupportedFormat: meaning = "unsupported-format"; break;
        case NVSDK_NGX_Result_FAIL_UnsupportedParameter: meaning = "unsupported-parameter"; break;
        default: break;
    }
    std::cout << "{\"stage\":" << json(stage) << ",\"ngx_result\":"
              << json(hex_code(uint32_t(result))) << ",\"meaning\":" << json(meaning) << "}" << std::endl;
    if (NVSDK_NGX_FAILED(result)) throw std::runtime_error(std::string(stage) + ": NGX " + hex_code(uint32_t(result)));
}

struct SdkState {
    ID3D11Device* device = nullptr;
    NVSDK_NGX_Parameter* parameters = nullptr;
    NVSDK_NGX_Handle* feature = nullptr;
    bool initialized = false;
    bool close() noexcept {
        bool ok = true;
        const auto release = [&](const char* stage, NVSDK_NGX_Result result) {
            std::cout << "{\"stage\":" << json(stage) << ",\"ngx_result\":"
                      << json(hex_code(uint32_t(result))) << "}" << std::endl;
            ok = ok && !NVSDK_NGX_FAILED(result);
        };
        if (feature) { release("release-feature", NVSDK_NGX_D3D11_ReleaseFeature(feature)); feature = nullptr; }
        // GetCapabilityParameters maps are app-owned according to installed nvsdk_ngx.h.
        if (parameters) { release("destroy-parameters", NVSDK_NGX_D3D11_DestroyParameters(parameters)); parameters = nullptr; }
        if (initialized) { release("shutdown-device", NVSDK_NGX_D3D11_Shutdown1(device)); initialized = false; }
        return ok;
    }
    ~SdkState() { if (initialized || parameters || feature) close(); }
};

static std::vector<uint8_t> readback(ID3D11Device* device, ID3D11DeviceContext* context,
                                     ID3D11Texture2D* texture, ID3D11Texture2D* reusable_staging = nullptr) {
    D3D11_TEXTURE2D_DESC desc{};
    texture->GetDesc(&desc);
    const UINT width = desc.Width, height = desc.Height;
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
    desc.MiscFlags = 0;
    ComPtr<ID3D11Texture2D> staging;
    if (!reusable_staging) {
        hr_check("create-readback", device->CreateTexture2D(&desc, nullptr, &staging));
        reusable_staging = staging.Get();
    }
    context->CopyResource(reusable_staging, texture);
    D3D11_MAPPED_SUBRESOURCE mapped{};
    hr_check("map-readback", context->Map(reusable_staging, 0, D3D11_MAP_READ, 0, &mapped));
    std::vector<uint8_t> result(size_t(width) * height * 4);
    for (UINT y = 0; y < height; ++y)
        std::memcpy(result.data() + size_t(y) * width * 4,
                    static_cast<const uint8_t*>(mapped.pData) + size_t(y) * mapped.RowPitch, size_t(width) * 4);
    context->Unmap(reusable_staging, 0);
    return result;
}
static void write_raw(const fs::path& file, const std::vector<uint8_t>& bytes) {
    std::ofstream out(file, std::ios::binary);
    out.write(reinterpret_cast<const char*>(bytes.data()), std::streamsize(bytes.size()));
    if (!out) throw std::runtime_error("raw output write failed");
}

static void record_runtime_modules() {
    const HANDLE snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, GetCurrentProcessId());
    if (snapshot == INVALID_HANDLE_VALUE) throw std::runtime_error("module inventory failed");
    MODULEENTRY32W module{};
    module.dwSize = sizeof(module);
    BOOL present = Module32FirstW(snapshot, &module);
    while (present) {
        std::wstring name = module.szModule;
        std::transform(name.begin(), name.end(), name.begin(), [](wchar_t c) { return std::towlower(c); });
        std::wstring path = module.szExePath;
        std::transform(path.begin(), path.end(), path.begin(), [](wchar_t c) { return std::towlower(c); });
        // NGX may map the exact SDK VSR module from its driver-managed cache
        // under a .bin name. Consumers must hash the observed module, not infer
        // identity from its filename or cache version directory.
        if (name.find(L"nvngx") != std::wstring::npos || path.find(L"\\nvidia\\ngx\\models\\vsr\\") != std::wstring::npos)
            std::cout << "{\"stage\":\"loaded-runtime\",\"name\":" << json(utf8(module.szModule))
                      << ",\"path\":" << json(utf8(module.szExePath)) << "}" << std::endl;
        present = Module32NextW(snapshot, &module);
    }
    CloseHandle(snapshot);
}
