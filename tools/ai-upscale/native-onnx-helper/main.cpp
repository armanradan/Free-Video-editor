// Optional Windows DirectML helper. Source only; no NVIDIA SDK APIs/binaries.
// Build against the pinned official ONNX Runtime 1.23 C API headers.
#define NOMINMAX
#include <windows.h>
#include <dxgi1_4.h>
#include <bcrypt.h>
#include <io.h>
#include <fcntl.h>
// The official header's non-MSVC SAL stubs conflict with MinGW's Windows SAL.
// Select only its existing-SAL branch while including it; keep the downloaded
// header unchanged. Both compilers use the Windows x64 ABI here.
#if defined(__MINGW32__) && !defined(_MSC_VER)
#define _MSC_VER 1930
#define _stdcall __stdcall
#define DIAXUS_ORT_MINGW_SAL
#ifndef _Frees_ptr_opt_
#define _Frees_ptr_opt_
#endif
#endif
#include <onnxruntime_c_api.h>
#ifdef DIAXUS_ORT_MINGW_SAL
#undef _MSC_VER
#undef _stdcall
#undef ORT_API_CALL
#define ORT_API_CALL __stdcall
#undef DIAXUS_ORT_MINGW_SAL
#endif
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace fs=std::filesystem;
constexpr const char* MODEL_SHA="1793a6e7fdf15a53eed213ba269ea768b5373858ae433d56ee8e1b0424377cc5";
constexpr uint32_t REQUEST=0x41495131,RESPONSE=0x41494f31,VERSION=2;

void require(bool value,const std::string& message){if(!value)throw std::runtime_error(message);}
void hr(HRESULT value,const char* boundary){if(FAILED(value))throw std::runtime_error(std::string(boundary)+" HRESULT="+std::to_string(static_cast<uint32_t>(value)));}
template<class Function> Function symbol(HMODULE library,const char* name){
    // Windows documents FARPROC as the dynamically retrieved function address.
    // Copy its representation into the exact C API signature, without calling
    // through the generic FARPROC type or suppressing incompatible-call warnings.
    auto address=GetProcAddress(library,name);Function result=nullptr;
    static_assert(sizeof(result)==sizeof(address));std::memcpy(&result,&address,sizeof(result));return result;
}
template<class T> struct Com {
    T* p=nullptr;~Com(){if(p)p->Release();}
    Com()=default;Com(const Com&)=delete;Com& operator=(const Com&)=delete;
    T** put(){return &p;}T* operator->()const{return p;}
};
std::string utf8(const std::wstring& text){
    int size=WideCharToMultiByte(CP_UTF8,0,text.c_str(),static_cast<int>(text.size()),nullptr,0,nullptr,nullptr);
    require(size>=0,"UTF-8 conversion failed");std::string result(size,'\0');
    WideCharToMultiByte(CP_UTF8,0,text.c_str(),static_cast<int>(text.size()),result.data(),size,nullptr,nullptr);return result;
}
std::string quoted(const std::string& value){
    std::ostringstream out;out<<'"';for(unsigned char c:value){
        if(c=='"'||c=='\\')out<<'\\'<<c;else if(c<32)out<<"\\u"<<std::hex<<std::setw(4)<<std::setfill('0')<<unsigned(c);else out<<c;
    }out<<'"';return out.str();
}
std::vector<uint8_t> read_file(const fs::path& path,size_t limit){
    std::ifstream in(path,std::ios::binary);require(bool(in),"Cannot open "+utf8(path.wstring()));
    std::vector<uint8_t> data;std::array<char,65536> block{};
    while(in){in.read(block.data(),block.size());auto n=static_cast<size_t>(in.gcount());
        require(n<=limit-data.size(),"File exceeds bound");data.insert(data.end(),block.data(),block.data()+n);}
    require(in.eof(),"File read failed");return data;
}
std::string sha256(const std::vector<uint8_t>& data){
    BCRYPT_ALG_HANDLE algorithm=nullptr;BCRYPT_HASH_HANDLE hash=nullptr;
    require(BCryptOpenAlgorithmProvider(&algorithm,BCRYPT_SHA256_ALGORITHM,nullptr,0)>=0,"SHA-256 provider failed");
    struct Guard{BCRYPT_ALG_HANDLE& a;BCRYPT_HASH_HANDLE& h;~Guard(){if(h)BCryptDestroyHash(h);if(a)BCryptCloseAlgorithmProvider(a,0);}}guard{algorithm,hash};
    require(BCryptCreateHash(algorithm,&hash,nullptr,0,nullptr,0,0)>=0,"SHA-256 hash failed");
    require(BCryptHashData(hash,const_cast<PUCHAR>(data.data()),static_cast<ULONG>(data.size()),0)>=0,"SHA-256 update failed");
    std::array<unsigned char,32> digest{};require(BCryptFinishHash(hash,digest.data(),digest.size(),0)>=0,"SHA-256 finish failed");
    std::ostringstream out;out<<std::hex<<std::setfill('0');for(auto b:digest)out<<std::setw(2)<<unsigned(b);return out.str();
}

struct Runtime {
    HMODULE library=nullptr;DLL_DIRECTORY_COOKIE directory=nullptr;
    const OrtApi* api=nullptr;OrtEnv* env=nullptr;OrtSessionOptions* options=nullptr;
    OrtSession* session=nullptr;OrtMemoryInfo* cpu=nullptr;OrtAllocator* allocator=nullptr;
    Com<IDXGIFactory1> factory;Com<IDXGIAdapter1> adapter;Com<IDXGIAdapter3> memory;
    DXGI_ADAPTER_DESC1 desc{};uint32_t ordinal;uint64_t upload=0,download=0,sampled_peak=0;
    explicit Runtime(const fs::path& root,const fs::path& model,uint32_t index,const std::wstring& name,const fs::path& profile):ordinal(index){
        try {
            hr(CreateDXGIFactory1(__uuidof(IDXGIFactory1),reinterpret_cast<void**>(factory.put())),"CreateDXGIFactory1");
            hr(factory->EnumAdapters1(index,adapter.put()),"EnumAdapters1");hr(adapter->GetDesc1(&desc),"GetDesc1");
            require(!(desc.Flags&DXGI_ADAPTER_FLAG_SOFTWARE)&&name==desc.Description,"Exact requested hardware DXGI adapter not selected");
            hr(adapter->QueryInterface(__uuidof(IDXGIAdapter3),reinterpret_cast<void**>(memory.put())),"IDXGIAdapter3");
            auto bytes=read_file(model,8*1024*1024);require(bytes.size()==4866413&&sha256(bytes)==MODEL_SHA,"Model size/hash is not allowlisted");
            const auto absolute=fs::absolute(root);
            require(SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS|LOAD_LIBRARY_SEARCH_USER_DIRS),"SetDefaultDllDirectories failed");
            directory=AddDllDirectory(absolute.c_str());require(directory!=nullptr,"AddDllDirectory failed");
            library=LoadLibraryExW((absolute/L"onnxruntime.dll").c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
            require(library!=nullptr,"ONNX Runtime library load failed, Windows error="+std::to_string(GetLastError()));
            using GetBase=const OrtApiBase*(ORT_API_CALL*)();auto get=symbol<GetBase>(library,"OrtGetApiBase");
            require(get!=nullptr,"OrtGetApiBase missing");auto base=get();require(std::string(base->GetVersionString())=="1.23.0","Wrong ONNX Runtime C API version");
            api=base->GetApi(ORT_API_VERSION);require(api!=nullptr,"C API 23 unavailable");
            check(api->CreateEnv(ORT_LOGGING_LEVEL_WARNING,"diaxus-native-ai",&env));
            check(api->CreateSessionOptions(&options));check(api->DisableMemPattern(options));
            check(api->SetSessionExecutionMode(options,ORT_SEQUENTIAL));
            check(api->AddSessionConfigEntry(options,"session.disable_cpu_ep_fallback","1"));
            if(!profile.empty())check(api->EnableProfiling(options,profile.c_str()));
            using Append=OrtStatus*(ORT_API_CALL*)(OrtSessionOptions*,int);
            auto append=symbol<Append>(library,"OrtSessionOptionsAppendExecutionProvider_DML");
            require(append!=nullptr,"DmlExecutionProvider export unavailable");check(append(options,static_cast<int>(index)));
            admit();check(api->CreateSessionFromArray(env,bytes.data(),bytes.size(),options,&session));
            check(api->CreateCpuMemoryInfo(OrtArenaAllocator,OrtMemTypeDefault,&cpu));check(api->GetAllocatorWithDefaultOptions(&allocator));
        }catch(...){cleanup();throw;}
    }
    ~Runtime(){cleanup();}
    void cleanup(){
        if(api){if(cpu)api->ReleaseMemoryInfo(cpu);if(session)api->ReleaseSession(session);if(options)api->ReleaseSessionOptions(options);if(env)api->ReleaseEnv(env);}
        cpu=nullptr;session=nullptr;options=nullptr;env=nullptr;
        if(library){FreeLibrary(library);}library=nullptr;
        if(directory){RemoveDllDirectory(directory);}directory=nullptr;
    }
    void check(OrtStatus* status){if(status){std::string message=api->GetErrorMessage(status);auto code=api->GetErrorCode(status);api->ReleaseStatus(status);throw std::runtime_error("ORT code="+std::to_string(code)+": "+message);}}
    void admit(){
        DXGI_QUERY_VIDEO_MEMORY_INFO info{};hr(memory->QueryVideoMemoryInfo(0,DXGI_MEMORY_SEGMENT_GROUP_LOCAL,&info),"QueryVideoMemoryInfo");
        sampled_peak=std::max(sampled_peak,info.CurrentUsage);
        constexpr uint64_t reserve=256ull*1024*1024,limit=1536ull*1024*1024;
        require(info.CurrentUsage<=limit-reserve&&info.Budget>=info.CurrentUsage&&info.Budget-info.CurrentUsage>=reserve,
            "DirectML memory admission rejected before allocation (256 MiB estimate, 1536 MiB process-local guard and OS budget)");
    }
    std::vector<float> infer(std::vector<float>& input,uint32_t width,uint32_t height){
        require(width&&height&&width<=257&&height<=259&&input.size()==size_t(width)*height*3,"Tensor dimensions outside diagnostic bound");admit();
        OrtValue* tensor=nullptr;OrtIoBinding* binding=nullptr;OrtValue** values=nullptr;size_t count=0;
        struct Guard{Runtime& r;OrtValue*& t;OrtIoBinding*& b;OrtValue**& v;size_t& n;~Guard(){
            if(v){for(size_t i=0;i<n;i++)r.api->ReleaseValue(v[i]);r.allocator->Free(r.allocator,v);}
            if(b){r.api->ReleaseIoBinding(b);}if(t){r.api->ReleaseValue(t);}
        }}guard{*this,tensor,binding,values,count};
        std::array<int64_t,4> shape{1,3,height,width};
        check(api->CreateTensorWithDataAsOrtValue(cpu,input.data(),input.size()*4,shape.data(),4,ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT,&tensor));
        check(api->CreateIoBinding(session,&binding));check(api->BindInput(binding,"input",tensor));
        check(api->BindOutputToDevice(binding,"output",cpu));check(api->RunWithBinding(session,nullptr,binding));
        check(api->SynchronizeBoundOutputs(binding));check(api->GetBoundOutputValues(binding,allocator,&values,&count));
        require(count==1,"Wrong output count");OrtTensorTypeAndShapeInfo* type=nullptr;check(api->GetTensorTypeAndShape(values[0],&type));
        struct TypeGuard{const OrtApi* a;OrtTensorTypeAndShapeInfo* p;~TypeGuard(){a->ReleaseTensorTypeAndShapeInfo(p);}}type_guard{api,type};
        std::array<int64_t,4> dims{};size_t rank=0;check(api->GetDimensionsCount(type,&rank));require(rank==4,"Wrong output rank");
        check(api->GetDimensions(type,dims.data(),4));require(dims==std::array<int64_t,4>{1,3,height*4,width*4},"Wrong output geometry");
        const OrtMemoryInfo* output_memory=nullptr;const char* memory_name=nullptr;
        check(api->GetTensorMemoryInfo(values[0],&output_memory));check(api->MemoryInfoGetName(output_memory,&memory_name));
        require(std::string(memory_name)=="Cpu","Output was not explicitly CPU-bound");
        float* data=nullptr;check(api->GetTensorMutableData(values[0],reinterpret_cast<void**>(&data)));
        size_t length=input.size()*16;std::vector<float> result(data,data+length);
        require(std::all_of(result.begin(),result.end(),[](float v){return std::isfinite(v);}),"Nonfinite model output");
        upload+=input.size()*4;download+=length*4;admit();return result;
    }
    void hello(){std::cout<<"{\"protocol\":2,\"pid\":"<<GetCurrentProcessId()<<",\"runtime\":\"1.23.0\",\"model_sha256\":"<<quoted(MODEL_SHA)
        <<",\"dxgi_index\":"<<ordinal<<",\"adapter_name\":"<<quoted(utf8(desc.Description))<<",\"vendor_id\":"<<desc.VendorId
        <<",\"device_id\":"<<desc.DeviceId<<",\"luid_high\":"<<static_cast<uint32_t>(desc.AdapterLuid.HighPart)
        <<",\"luid_low\":"<<desc.AdapterLuid.LowPart<<",\"cpu_fallback\":false,\"tile_interior\":128,\"halo\":34}"<<std::endl;}
};

std::vector<float> npy(const fs::path& path,size_t expected){
    auto bytes=read_file(path,16*1024*1024);require(bytes.size()>=10&&std::string(reinterpret_cast<char*>(bytes.data()),6)=="\x93NUMPY"&&bytes[6]==1,"Expected NPY v1");
    size_t length=bytes[8]+size_t(bytes[9])*256;require(length<=bytes.size()-10,"Truncated NPY header");
    std::string header(reinterpret_cast<char*>(bytes.data()+10),length);
    require(header.find("'<f4'")!=std::string::npos&&header.find("'fortran_order': False")!=std::string::npos,"Expected contiguous little-endian FP32 NPY");
    require(bytes.size()-10-length==expected*4,"Wrong NPY payload length");std::vector<float> result(expected);
    std::memcpy(result.data(),bytes.data()+10+length,expected*4);return result;
}
void numeric(Runtime& runtime,const fs::path& reference){
    std::cout<<"{\"status\":\"running\",\"cases\":[";bool first=true;
    for(auto [w,h]:std::array<std::pair<uint32_t,uint32_t>,5>{{{1,1},{3,5},{17,19},{33,31},{257,259}}}){
        auto stem=std::to_string(w)+"x"+std::to_string(h);auto input=npy(reference/(stem+"-input.npy"),size_t(w)*h*3);
        auto expected=npy(reference/(stem+"-fp32.npy"),input.size()*16);auto actual=runtime.infer(input,w,h);
        double sum=0,max=0;for(size_t i=0;i<actual.size();i++){double e=std::abs(double(actual[i])-expected[i]);sum+=e;max=std::max(max,e);}
        require(max<=.01&&sum/actual.size()<=.001,"FP32 reference gate failed");
        if(!first){std::cout<<',';}first=false;std::cout<<"{\"shape\":"<<quoted(stem)<<",\"max_abs\":"<<max<<",\"mae\":"<<sum/actual.size()<<'}';
    }std::cout<<"],\"host_to_gpu_bytes\":"<<runtime.upload<<",\"gpu_to_host_bytes\":"<<runtime.download
        <<",\"sampled_process_local_peak\":"<<runtime.sampled_peak<<",\"result\":\"passed-numeric-pilot\"}"<<std::endl;
}
void read_exact(void* destination,size_t size){std::cin.read(static_cast<char*>(destination),size);require(size_t(std::cin.gcount())==size,"Truncated helper request");}
void serve(Runtime& runtime){
    for(;;){std::array<uint32_t,6> header{};std::cin.read(reinterpret_cast<char*>(header.data()),sizeof(header));
        if(std::cin.eof()&&std::cin.gcount()==0){return;}
        require(size_t(std::cin.gcount())==sizeof(header),"Truncated request header");
        require(header[0]==REQUEST&&header[1]==VERSION,"Wrong protocol magic/version");
        if(header[2]==2){require(header[3]==0&&header[4]==0&&header[5]==0,"Malformed stop request");return;}
        require(header[2]==1&&header[3]&&header[4]&&header[3]<=196&&header[4]<=196&&header[5]==header[3]*header[4]*3*4,"Invalid bounded FP32 tile request");
        std::vector<float> source(header[5]/4);read_exact(source.data(),header[5]);auto before_up=runtime.upload,before_down=runtime.download;
        require(std::all_of(source.begin(),source.end(),[](float v){return std::isfinite(v)&&v>=0.f&&v<=1.f;}),"Tile input must be normalized FP32 RGB");
        auto result=runtime.infer(source,header[3],header[4]);
        std::array<uint32_t,4> response{RESPONSE,VERSION,0,static_cast<uint32_t>(result.size()*4)};
        std::array<uint64_t,3> metrics{runtime.upload-before_up,runtime.download-before_down,runtime.sampled_peak};
        std::cout.write(reinterpret_cast<char*>(response.data()),sizeof(response));std::cout.write(reinterpret_cast<char*>(metrics.data()),sizeof(metrics));
        std::cout.write(reinterpret_cast<char*>(result.data()),result.size()*4);std::cout.flush();require(bool(std::cout),"Helper response pipe closed");
    }
}
int wmain(int argc,wchar_t** argv){
    _setmode(_fileno(stdin),_O_BINARY);_setmode(_fileno(stdout),_O_BINARY);
    try{
        require(argc==6||argc==8,"expected runtime-directory model.onnx DXGI-index exact-adapter-name profile-prefix-or-dash [--numeric reference-directory]");
        auto index=std::stoul(argv[3]);require(index<32,"DXGI ordinal exceeds discovery bound");
        Runtime runtime(argv[1],argv[2],static_cast<uint32_t>(index),argv[4],std::wstring(argv[5])==L"-"?fs::path():fs::path(argv[5]));
        runtime.hello();if(argc==8){require(std::wstring(argv[6])==L"--numeric","Unknown diagnostic mode");numeric(runtime,argv[7]);}else serve(runtime);
        return 0;
    }catch(const std::exception& e){std::cerr<<"AI_HELPER_ERROR: "<<e.what()<<std::endl;return 1;}
}
