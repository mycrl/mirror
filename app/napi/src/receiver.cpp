#include "./receiver.h"
#include "./context.h"

void ReceiverService::Create(Napi::Env env, Napi::Object exports)
{
    auto props =
    {
        InstanceMethod<&ReceiverService::Close>("close"),
    };

    exports.Set("ReceiverService", DefineClass(env, "ReceiverService", props));
}

ReceiverService::ReceiverService(const Napi::CallbackInfo& info) : Napi::ObjectWrap<ReceiverService>(info)
{
    auto env = info.Env();
    auto context = env.GetInstanceData<Context>();
    auto id = info[0].As<Napi::Number>().Uint32Value();

    if (context->mirror == nullptr)
    {
        Napi::Error::New(env, "mirror is null").ThrowAsJavaScriptException();
        return;
    }

    FrameSink sink;
    sink.ctx = this;
    sink.video = ReceiverService::_video_proc;
    sink.audio = ReceiverService::_audio_proc;
    sink.close = ReceiverService::_close_proc;
    _receiver = mirror_create_receiver(context->mirror, id, sink);
    if (_receiver == nullptr)
    {
        Napi::Error::New(env, "create receiver failed").ThrowAsJavaScriptException();
        return;
    }

    _callback = ThreadSafeFunction::New(env,
                                        info[1].As<Napi::Function>(),
                                        "Callback",
                                        0,
                                        1,
                                        new Ref(Persistent(info.This())),
                                        [](Napi::Env, void*, Ref* ctx)
                                        {
                                            delete ctx;
                                        });
}

Napi::Value ReceiverService::Close(const Napi::CallbackInfo& info)
{
    if (_renderer != nullptr)
    {
        renderer_destroy(_renderer);
        _renderer = nullptr;
    }

    if (_receiver != nullptr)
    {
        mirror_receiver_destroy(_receiver);
        _receiver = nullptr;
    }

    return info.Env().Undefined();
}

bool ReceiverService::_video_proc(void* ctx, VideoFrame* frame)
{
    auto self = (ReceiverService*)ctx;
    if (self->_receiver == nullptr)
    {
        return false;
    }

    if (self->_thread == nullptr)
    {
        self->_thread = new std::thread(
            [=]()
            {
                int width = GetSystemMetrics(SM_CXSCREEN);
                int height = GetSystemMetrics(SM_CYSCREEN);

                HINSTANCE hinstance = (HINSTANCE)GetModuleHandle(nullptr);
                WNDCLASSEX wcex;
                wcex.cbSize = sizeof(WNDCLASSEX);
                wcex.style = CS_HREDRAW | CS_VREDRAW;
                wcex.lpfnWndProc = ReceiverService::_wnd_proc;
                wcex.cbClsExtra = 0;
                wcex.cbWndExtra = 0;
                wcex.hInstance = hinstance;
                wcex.hIcon = LoadIcon(hinstance, IDI_APPLICATION);
                wcex.hCursor = LoadCursor(nullptr, IDC_ARROW);
                wcex.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);
                wcex.lpszMenuName = nullptr;
                wcex.lpszClassName = "mirror remote casting frame";
                wcex.hIconSm = LoadIcon(wcex.hInstance, IDI_APPLICATION);
                if (!RegisterClassEx(&wcex))
                {
                    return;
                }

                HWND hwnd = CreateWindow("mirror remote casting frame",
                                         "mirror remote casting frame",
                                         WS_OVERLAPPEDWINDOW,
                                         0,
                                         0,
                                         width,
                                         height,
                                         nullptr,
                                         nullptr,
                                         hinstance,
                                         nullptr);
                if (!hwnd)
                {
                    return;
                }

                SetWindowLong(hwnd, GWL_STYLE, 0);
                SetWindowPos(hwnd, HWND_TOP, 0, 0, width, height, SWP_FRAMECHANGED);

                ShowWindow(hwnd, SW_SHOW);
                UpdateWindow(hwnd);

                Size size;
                size.width = width;
                size.height = height;

                auto handle = renderer_create_window_handle(hwnd, hinstance);
                self->_renderer = renderer_create(size, handle);

                MSG msg;
                while (GetMessage(&msg, NULL, 0, 0))
                {
                    TranslateMessage(&msg);
                    DispatchMessage(&msg);
                }

                renderer_window_handle_destroy(handle);
                DestroyWindow(hwnd);

                self->_thread = nullptr;
            });
    }

    if (self->_renderer == nullptr)
    {
        return true;
    }

    return renderer_on_video(self->_renderer, frame);
}

bool ReceiverService::_audio_proc(void* ctx, AudioFrame* frame)
{
    auto self = (ReceiverService*)ctx;
    if (self->_receiver == nullptr)
    {
        return false;
    }

    if (self->_renderer == nullptr)
    {
        return true;
    }

    return renderer_on_audio(self->_renderer, frame);
}

void ReceiverService::_close_proc(void* ctx)
{
    PostQuitMessage(0);

    auto self = (ReceiverService*)ctx;
    self->_callback.BlockingCall();
    self->_callback.Release();
}

LRESULT CALLBACK ReceiverService::_wnd_proc(HWND hwnd,
                                            UINT message,
                                            WPARAM wparam,
                                            LPARAM lparam)
{
    switch (message)
    {
        case WM_DESTROY:
            PostQuitMessage(0);
            break;
        default:
            return DefWindowProc(hwnd, message, wparam, lparam);
            break;
    }

    return 0;
}

void ReceiverService::_callback_proc(Napi::Env env,
                                     Napi::Function callback,
                                     Ref* context,
                                     void* data)
{
    if (env == nullptr || callback == nullptr)
    {
        return;
    }

    callback.Call(context->Value(), {});
}