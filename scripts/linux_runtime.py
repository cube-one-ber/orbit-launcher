"""Libraries that must follow the host's graphics drivers and desktop configuration."""

import re


# Keep these outside the bundle, including when a Qt SDK supplies a copy. New
# Mesa/NVIDIA drivers can require newer Wayland, GBM, DRM or C++ runtime symbols
# than the build machine provides. Fontconfig must match the host's font rules.
# GLib remains bundled as one family because its dynamically loaded modules can
# otherwise mix incompatible versions. Qt/Kirigami libraries are always bundled.
HOST_LIBRARIES = re.compile(
    r"^(ld-linux|libnss_|"
    r"lib(c|m|mvec|dl|pthread|rt|resolv|util|anl|thread_db)\.so|"
    r"lib(EGL|GL|GLX|GLdispatch|OpenGL|GLESv1_CM|GLESv2|glapi|gbm|vulkan)\.so|"
    r"libdrm(?:_[a-z0-9_]+)?\.so|libnvidia-|"
    r"libwayland-(client|cursor|egl|server)\.so|"
    r"lib(X11|X11-xcb|xcb|xcb-glx|xcb-dri2|xcb-dri3)\.so|"
    r"lib(fontconfig|freetype|harfbuzz|expat|stdc\+\+|gcc_s|gomp)\.so)"
)


def host_library(name):
    return bool(HOST_LIBRARIES.match(name))
