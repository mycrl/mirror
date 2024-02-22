#include <obs.h>
#include <unistd.h>

bool enum_sources_fn(void* ctx, obs_source_t* source)
{
    const char* name = obs_source_get_name(source);
    const char* id = obs_source_get_id(source);
    printf("Device: Name: %s, ID: %s\n", name, id);
}

int main() 
{
    if (!obs_startup("zh-CN", NULL, NULL))
    {
        return -1;
    }
    
    obs_enum_sources(enum_sources_fn, NULL);
    
    sleep(5);
    obs_shutdown();
    return 0;
}
