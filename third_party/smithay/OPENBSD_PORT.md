# Meridian OpenBSD Smithay port layer

This directory is based on upstream Smithay commit
`060e9cd9c8ae2633947bdb72cffa1748ac53922d`, the revision Meridian previously
referenced directly from GitHub.

Meridian carries this source temporarily because Cargo cannot apply a small
target patch to a Git dependency. Keep changes minimal and suitable for an
upstream pull request.

The upstream nested `[workspace]`, development-only dependencies, examples,
benchmarks and local profile are omitted so this package can be used as a lean
path dependency inside Meridian's workspace. Smithay's library source and
runtime/build dependency versions otherwise remain at the pinned revision.
The vendored package's default feature set is empty because Meridian declares
its required Smithay features explicitly; this avoids pulling unrelated
Vulkan/Pixman defaults into workspace-wide checks.

## Native OpenBSD difference

`linux-drm-syncobj-v1` is not compiled on OpenBSD. Its implementation requires
Linux `eventfd(2)` and the DRM syncobj-eventfd ioctl. Neither is an OpenBSD
interface, and substituting a pipe or kqueue object would not implement the
kernel contract expected by the ioctl.

This does **not** disable Smithay's DRM/KMS, GBM or EGL backends. OpenBSD keeps
the native DRM rendering path and simply does not advertise the Linux explicit
synchronization protocol. Clients must use the synchronization paths actually
available on OpenBSD.

OpenBSD exposes EGL as the ABI-versioned `libEGL.so.2.0`, while upstream
Smithay loads the Linux soname `libEGL.so.1`. The port layer selects the native
OpenBSD soname at compile time; it does not create compatibility symlinks.

## Updating

When updating Smithay:

1. import the new upstream revision without local history;
2. reapply only the OpenBSD target boundary;
3. run Linux workspace checks to ensure behavior is unchanged there;
4. run `cargo check -p meridian-wm` and `cargo check -p meridian-compositor` on
   OpenBSD;
5. remove this vendored copy once an equivalent upstream boundary is available.
