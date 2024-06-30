//
//  camera.cpp
//  capture
//
//  Created by Panda on 2024/6/30.
//

#ifdef WIN32

#include "camera.h"

CameraCapture::CameraCapture()
{
    MFStartup(MF_VERSION);
}

CameraCapture::~CameraCapture()
{
    MFShutdown();
}

int CameraCapture::EnumDevices(struct DeviceList* list)
{
    IMFAttributes* attributes;
    auto ret = MFCreateAttributes(&attributes, 1);
    if (FAILED(ret))
    {
        return -1;
    }

    ret = attributes->SetGUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                              MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);
    if (FAILED(ret))
    {
        attributes->Release();
        return -2;
    }

    UINT32 count;
    IMFActivate** devices;
    ret = MFEnumDeviceSources(attributes, &devices, &count);
    if (FAILED(ret) || count == 0)
    {
        attributes->Release();
        return -4;
    }

    for (UINT32 i = 0; i < count; i++)
    {
        WCHAR* name = NULL;
        WCHAR* symlink = NULL;
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

int CameraCapture::StartCapture(const char* id,
                 int width,
                 int height,
                 int fps,
                 std::function<void(VideoFrame*)> callback)
{
    IMFAttributes* attributes;
    auto ret = MFCreateAttributes(&attributes, 1);
    if (FAILED(ret))
    {
        return -1;
    }

    ret = attributes->SetGUID(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                              MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID);
    if (FAILED(ret))
    {
        return -2;
    }

    ret = attributes->SetUINT32(MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, true);
    if (FAILED(ret))
    {
        attributes->Release();
        return -2;
    }

    ret = attributes->SetUINT32(MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, true);
    if (FAILED(ret))
    {
        return -2;
    }

    WCHAR name[1024];
    MultiByteToWideChar(CP_UTF8, 0, id, -1, name, 1024);
    ret = attributes->SetString(MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, name);
    if (FAILED(ret))
    {
        return -3;
    }

    IMFMediaSource* device;
    ret = MFCreateDeviceSource(attributes, &device);
    if (FAILED(ret))
    {
        return -4;
    }

    IMFSourceReader* reader = NULL;
    ret = MFCreateSourceReaderFromMediaSource(device, attributes, &reader);
    if (FAILED(ret))
    {
        return -5;
    }

    device->Release();
    attributes->Release();

    IMFMediaType* type;
    ret = MFCreateMediaType(&type);
    if (FAILED(ret))
    {
        return -6;
    }

    ret = type->SetGUID(MF_MT_MAJOR_TYPE, MFMediaType_Video);
    if (FAILED(ret))
    {
        return -7;
    }

    ret = type->SetGUID(MF_MT_SUBTYPE, MFVideoFormat_NV12);
    if (FAILED(ret))
    {
        return -8;
    }

    ret = type->SetUINT32(MF_MT_DEFAULT_STRIDE, width);
    if (FAILED(ret))
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

    reader->AddRef();
    ret = reader->SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                      NULL,
                                      type);
    if (FAILED(ret))
    {
        return -15;
    }

    _is_runing = true;
    std::thread(
        [=]()
        {
            CRITICAL_SECTION critsec;
            InitializeCriticalSection(&critsec);

            VideoFrame frame = {};
            frame.rect.width = width;
            frame.rect.height = height;

            for (; this->_is_runing;)
            {
                EnterCriticalSection(&critsec);
                if (!_ReadSample(reader, &frame, callback))
                {
                    break;
                }
                else
                {
                    LeaveCriticalSection(&critsec);
                }
            }

            reader->Release();
            DeleteCriticalSection(&critsec);
        }).detach();

    type->Release();
    return 0;
}

void CameraCapture::StopCapture()
{
    _is_runing = false;
}

bool CameraCapture::_ReadSample(IMFSourceReader* reader,
                 VideoFrame* frame,
                 std::function<void(VideoFrame*)> callback)
{
    DWORD index;
    DWORD flags;
    LONGLONG timestamp;
    IMFSample* sample = NULL;
    auto ret = reader->ReadSample((DWORD)MF_SOURCE_READER_FIRST_VIDEO_STREAM,
                                  0,
                                  &index,
                                  &flags,
                                  &timestamp,
                                  &sample);
    if (FAILED(ret))
    {
        return false;
    }

    if (index != 0 || sample == NULL)
    {
        return true;
    }

    IMFMediaBuffer* buffer = NULL;
    ret = sample->ConvertToContiguousBuffer(&buffer);
    if (FAILED(ret))
    {
        return false;
    }

    IMF2DBuffer* texture = NULL;
    ret = buffer->QueryInterface(IID_IMF2DBuffer, (void**)&texture);
    if (FAILED(ret))
    {
        return false;
    }

    LONG stride;
    BYTE* data = NULL;
    ret = texture->Lock2D(&data, &stride);
    if (FAILED(ret) || data == NULL)
    {
        return false;
    }

    frame->linesize[0] = stride;
    frame->linesize[1] = stride;

    frame->data[0] = data;
    frame->data[1] = data + (stride * frame->rect.height);
    callback(frame);

    ret = texture->Unlock2D();
    if (FAILED(ret))
    {
        return false;
    }

    texture->Release();
    buffer->Release();
    sample->Release();
    return true;
}

char* CameraCapture::_WcharToString(WCHAR* src, int size)
{
    int len = WideCharToMultiByte(CP_UTF8,
                                  0,
                                  src,
                                  size,
                                  0,
                                  0,
                                  NULL,
                                  NULL);
    char* dst = (char*)malloc((len + 1) * sizeof(char));
    if (!dst)
    {
        return NULL;
    }

    WideCharToMultiByte(CP_UTF8,
                        0,
                        src,
                        size,
                        dst,
                        len,
                        NULL,
                        NULL);
    dst[len] = '\0';
    return dst;
}

#endif // WIN32