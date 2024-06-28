#pragma once

#include "capture.h"

#include <thread>
#include <functional>

#ifdef WIN32
#include <Windows.h>
#include <mfapi.h>
#include <mfidl.h>
#include <mfreadwrite.h>
#include <mftransform.h>
#include <shlwapi.h>

class VideoCapture : public IMFSourceReaderCallback
{
public:
    VideoCapture()
    {
        MFStartup(MF_VERSION);
    }

    ~VideoCapture()
    {
        delete _frame;
        MFShutdown();
    }

    static int EnumDevices(struct DeviceList* list)
    {
        IMFAttributes* attributes;
        auto ret = MFCreateAttributes(&attributes, 1);
        if (!SUCCEEDED(ret))
        {
            return -1;
        }

        ret = attributes->SetGUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                                  MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);
        if (!SUCCEEDED(ret))
        {
            attributes->Release();
            return -2;
        }

        ret = attributes->SetUINT32(MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1);
        if (!SUCCEEDED(ret))
        {
            attributes->Release();
            return -3;
        }

        UINT32 count;
        IMFActivate** devices;
        ret = MFEnumDeviceSources(attributes, &devices, &count);
        if (!SUCCEEDED(ret) || count == 0)
        {
            attributes->Release();
            return -4;
        }

        for (UINT32 i = 0; i < count; i++)
        {
            WCHAR* name = nullptr;
            WCHAR* symlink = nullptr;
            UINT32 name_size, symlink_size;
            ret = devices[i]->GetAllocatedString(MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
                                                 &name,
                                                 &name_size);
            if (SUCCEEDED(ret))
            {
                ret = devices[i]->GetAllocatedString(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
                                                     &symlink,
                                                     &symlink_size);
                if (SUCCEEDED(ret))
                {
                    struct DeviceDescription* device = new DeviceDescription{};
                    device->id = _WcharToString(symlink, symlink_size);
                    device->name = _WcharToString(name, name_size);
                    device->type = DeviceType::kDeviceTypeVideo;

                    list->devices[list->size] = device;
                    list->size++;
                }
            }

            devices[i]->Release();
            CoTaskMemFree(symlink);
            CoTaskMemFree(name);
        }

        attributes->Release();
        CoTaskMemFree(devices);
        return 0;
    }

    int StartCapture(const char* id,
                     int width,
                     int height,
                     int fps,
                     std::function<void(VideoFrame*)> callback)
    {
        _frame->rect.width = width;
        _frame->rect.height = height;
        _frame->linesize[0] = width;
        _frame->linesize[1] = width;
        _callback = callback;

        IMFAttributes* attributes;
        auto ret = MFCreateAttributes(&attributes, 1);
        if (!SUCCEEDED(ret))
        {
            return -1;
        }

        ret = attributes->SetGUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                                  MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);
        if (!SUCCEEDED(ret))
        {
            return -2;
        }

        ret = attributes->SetUINT32(MF_READWRITE_DISABLE_CONVERTERS, true);
        if (!SUCCEEDED(ret))
        {
            return -2;
        }

        ret = attributes->SetUnknown(MF_SOURCE_READER_ASYNC_CALLBACK, this);
        if (!SUCCEEDED(ret))
        {
            return -2;
        }

        WCHAR name[1024];
        MultiByteToWideChar(CP_UTF8, 0, id, -1, name, 1024);
        ret = attributes->SetString(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, name);
        if (!SUCCEEDED(ret))
        {
            return -3;
        }

        IMFMediaSource* device;
        ret = MFCreateDeviceSource(attributes, &device);
        if (!SUCCEEDED(ret))
        {
            return -4;
        }

        ret = MFCreateSourceReaderFromMediaSource(device, attributes, &_reader);
        if (!SUCCEEDED(ret))
        {
            return -5;
        }

        device->Release();
        attributes->Release();

        IMFMediaType* type;
        ret = MFCreateMediaType(&type);
        if (!SUCCEEDED(ret))
        {
            return -6;
        }

        ret = type->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video);
        if (!SUCCEEDED(ret))
        {
            return -7;
        }

        ret = type->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_NV12);
        if (!SUCCEEDED(ret))
        {
            return -8;
        }

        ret = MFSetAttributeSize(type, MF_MT_FRAME_SIZE, width, height);
        if (FAILED(ret))
        {
            return -9;
        }

        ret = MFSetAttributeRatio(type, MF_MT_FRAME_RATE, fps, 1);
        if (FAILED(ret))
        {
            return -10;
        }

        ret = _reader->SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM, nullptr, type);
        if (!SUCCEEDED(ret))
        {
            return -11;
        }

        _is_runing = true;
        ret = _reader->ReadSample((DWORD)MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                  0,
                                  nullptr,
                                  nullptr,
                                  nullptr,
                                  nullptr);
        if (!SUCCEEDED(ret))
        {
            return -12;
        }

        type->Release();
        return 0;
    }

    void StopCapture()
    {
        _is_runing = false;
        _reader->Release();
    }

    STDMETHODIMP QueryInterface(REFIID iid, void** ppv)
    {
        static const QITAB qit[] = { QITABENT(VideoCapture, IMFSourceReaderCallback),{ 0 }, };
        return QISearch(this, qit, iid, ppv);
    }

    STDMETHODIMP_(ULONG) AddRef()
    {
        return InterlockedIncrement(&_reference_count);
    }

    STDMETHODIMP_(ULONG) Release()
    {
        ULONG count = InterlockedDecrement(&_reference_count);
        if (count == 0)
        {
            delete this;
        }

        return count;
    }

    STDMETHODIMP OnReadSample(HRESULT status,
                              DWORD index,
                              DWORD flags,
                              LONGLONG timestamp,
                              IMFSample* sample)
    {
        // DebugBreak();
        if (!_is_runing)
        {
            return S_FALSE;
        }

        if (SUCCEEDED(status) && sample)
        {
            IMFMediaBuffer* buffer = nullptr;
            auto ret = sample->GetBufferByIndex(0, &buffer);
            if (!SUCCEEDED(ret))
            {
                return S_FALSE;
            }

            BYTE* frame = nullptr;
            ret = buffer->Lock(&frame, nullptr, nullptr);
            if (!SUCCEEDED(ret) || frame == nullptr)
            {
                return S_FALSE;
            }

            _frame->data[0] = frame;
            _frame->data[1] = frame + (_frame->rect.width * _frame->rect.height);

            _callback(_frame);
            buffer->Release();
        }

        auto ret = _reader->ReadSample((DWORD)MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                       0,
                                       nullptr,
                                       nullptr,
                                       nullptr,
                                       nullptr);
        if (!SUCCEEDED(ret))
        {
            return S_FALSE;
        }

        return S_OK;
    }

    STDMETHODIMP OnEvent(DWORD, IMFMediaEvent*)
    {
        return S_OK;
    }

    STDMETHODIMP OnFlush(DWORD)
    {
        return S_OK;
    }
private:
    std::function<void(VideoFrame*)> _callback;
    VideoFrame* _frame = new VideoFrame{};
    IMFSourceReader* _reader = nullptr;
    long _reference_count = 1;
    bool _is_runing = false;

    static char* _WcharToString(WCHAR* src, int size)
    {
        int length = WideCharToMultiByte(CP_UTF8,
                                         0,
                                         src,
                                         size,
                                         0,
                                         0,
                                         nullptr,
                                         nullptr);
        char* dst = (char*)malloc((length + 1) * sizeof(char));
        if (!dst)
        {
            return nullptr;
        }

        WideCharToMultiByte(CP_UTF8,
                            0,
                            src,
                            size,
                            dst,
                            length,
                            nullptr,
                            nullptr);
        dst[length] = '\0';
        return dst;
    }
};
#endif // WIN32