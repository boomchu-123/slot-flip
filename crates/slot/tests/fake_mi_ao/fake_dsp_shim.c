/* A stand-in for the console's preloaded libpadsp, for the one behaviour of it
 * that crashed exp-v0.4.8: how it answers `open`.
 *
 * The real shim keeps a pointer to the real `open`, fills it in on the first
 * `open` it passes on for some other path, and for the sound device (/dev/dsp
 * and its aliases) calls it without checking. A program whose first `open`
 * through the shim is the sound device jumps to address zero. This does the same,
 * so a test can show that slot makes an ordinary `open` first. A device that
 * passes that check is answered with /dev/null, which accepts what is written.
 *
 * Run with LD_PRELOAD pointing at it.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <fcntl.h>
#include <stdarg.h>
#include <string.h>
#include <sys/types.h>

static int (*real_open)(const char *, int, ...);

int open(const char *path, int flags, ...) {
	mode_t mode = 0;
	va_list ap;
	va_start(ap, flags);
	if (flags & O_CREAT) mode = va_arg(ap, mode_t);
	va_end(ap);

	if (!strcmp(path, "/dev/dsp")) {
		/* No check, as in the real one: an unprimed pointer is a crash. */
		return real_open("/dev/null", O_RDWR, 0666);
	}
	if (!real_open) real_open = (int (*)(const char *, int, ...))dlsym(RTLD_NEXT, "open");
	return real_open(path, flags, mode);
}
