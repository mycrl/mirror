#ifndef CONTEXT_H
#define CONTEXT_H
#pragma once

#include <napi.h>

extern "C"
{
#include <mirror.h>
}

class Context
{
public:
    Napi::ObjectReference exports;
    Mirror mirror = nullptr;

    static void Finalize(Napi::Env env, Context* self)
    {
        if (self->mirror != nullptr)
        {
            self->mirror = nullptr;
        }

        delete self;
    }
};

#endif // CONTEXT_H