#include <fcntl.h>
#include <xf86drmMode.h>
#include <gbm.h>
#include <vulkan/vulkan.h>
#include <vulkan/vulkan_core.h>
#include <vulkan/vulkan_xlib.h>

int main()
{
    int drm_fd = open("/dev/dri/card0", O_RDWR | O_CLOEXEC);
    drmModeRes *resources = drmModeGetResources(drm_fd);
    drmModeConnector *connector = drmModeGetConnector(drm_fd, resources->connectors[0]);
    drmModeEncoder *encoder = drmModeGetEncoder(drm_fd, connector->encoder_id);
    drmModeCrtc *crtc = drmModeGetCrtc(drm_fd, encoder->crtc_id);

    struct gbm_device *gbm = gbm_create_device(drm_fd);
    struct gbm_surface *surface = gbm_surface_create(gbm, width, height, GBM_FORMAT_XRGB8888,
                                                GBM_BO_USE_SCANOUT | GBM_BO_USE_RENDERING);

                                                // 创建 Vulkan 实例
    VkInstance instance;
    VkInstanceCreateInfo createInfo = {};
    createInfo.sType = VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO;
    vkCreateInstance(&createInfo, nullptr, &instance);

    VkPhysicalDevice physicalDevice;
    vkEnumeratePhysicalDevices(instance, &physicalDeviceCount, &physicalDevice);
    VkDevice device;
    VkDeviceCreateInfo deviceCreateInfo = {};
    vkCreateDevice(physicalDevice, &deviceCreateInfo, nullptr, &device);

    struct gbm_bo *bo = gbm_surface_lock_front_buffer(surface);
    int fd = gbm_bo_get_fd(bo);

    VkImportMemoryFdInfoKHR importInfo = {};
    importInfo.sType = VK_STRUCTURE_TYPE_IMPORT_MEMORY_FD_INFO_KHR;
    importInfo.handleType = VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_FD_BIT_KHR;
    importInfo.fd = fd;

    VkMemoryAllocateInfo memAllocInfo = {};
    memAllocInfo.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO;
    memAllocInfo.pNext = &importInfo;

    VkDeviceMemory memory;
    vkAllocateMemory(device, &memAllocInfo, nullptr, &memory);

    // vkCmdCopyBufferToImage(commandBuffer, srcBuffer, dstImage, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, ...);

    uint32_t fb;
    drmModeAddFB(drm_fd, width, height, 24, 32, gbm_bo_get_stride(bo), gbm_bo_get_handle(bo).u32, &fb);
    drmModeSetCrtc(drm_fd, crtc->crtc_id, fb, 0, 0, &connector->connector_id, 1, &crtc->mode);
}
