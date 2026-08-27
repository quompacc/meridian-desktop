# OpenBSD quiet-boot boundary

OpenBSD 7.9/amd64 has no supported kernel or boot-loader quiet flag. Its
`boot(8)` command accepts `-a`, `-c`, `-d`, and `-s`; suppressing all kernel
text would require redirecting the system console or carrying base-system
patches. Meridian does neither, because both approaches weaken local recovery.

The supported Meridian path therefore keeps the loader, kernel, and early
`rc(8)` diagnostics intact, then switches to the getty-free graphical `ttyC4`
as soon as the native bootsplash package service starts. The splash owns DRM
until `meridian-login` requests its synchronous handover. SSH and `ttyC1`
remain available throughout as recovery paths.

This boundary preserves useful diagnostics in `/var/run/dmesg.boot`,
`/var/log/messages`, and the console message buffer while replacing the late
userspace service chatter and avoiding a text-console flash before the
greeter.
