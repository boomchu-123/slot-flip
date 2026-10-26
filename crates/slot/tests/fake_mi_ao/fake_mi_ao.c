/* A stand-in for the Mini Plus' libmi_ao.so, for running slot's vendor audio
 * backend through the real thread and the real dlopen path without a console.
 *
 * It answers the calls slot makes and appends what it was asked to the file
 * named by FAKE_MI_AO_LOG, one line per call, so a test can read it back.
 * FAKE_MI_AO_MODE picks how the device starts:
 *   fresh      no attributes yet (get fails)       -> slot sets its own
 *   configured already 48 kHz stereo, 1024 points  -> slot adopts it
 *   busy       set fails until the device is disabled
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { int rate, bits, work, sound; unsigned frames, points, codec, chn; int i2s[3]; } Attr;
typedef struct { int bits, sound; void *addr[16]; unsigned long long ts; unsigned seq, len, pool[2]; void *src[16]; unsigned srclen; } Frame;
typedef struct { unsigned total, free, busy; } State;

static Attr cur; static int have, enabled;
static unsigned long long sent_frames, sent_sum;

static void note(const char *fmt, ...) __attribute__((format(printf, 1, 2)));
#include <stdarg.h>
static void note(const char *fmt, ...) {
	const char *path = getenv("FAKE_MI_AO_LOG");
	if (!path) return;
	FILE *f = fopen(path, "a"); if (!f) return;
	va_list ap; va_start(ap, fmt); vfprintf(f, fmt, ap); va_end(ap);
	fputc('\n', f); fclose(f);
}
static const char *mode(void) { const char *m = getenv("FAKE_MI_AO_MODE"); return m ? m : "fresh"; }

int MI_AO_GetPubAttr(int dev, Attr *a) {
	(void)dev;
	if (!have && !strcmp(mode(), "configured")) {
		memset(&cur, 0, sizeof cur);
		cur.rate = 48000; cur.sound = 1; cur.frames = 6; cur.points = 1024; cur.chn = 2; have = 1;
	}
	if (!have) { note("get fail"); return 0x80000001; }
	*a = cur; note("get ok %d %u", cur.rate, cur.points); return 0;
}
int MI_AO_SetPubAttr(int dev, Attr *a) {
	(void)dev;
	if (!strcmp(mode(), "busy") && enabled) { note("set refused"); return 0xA0052005; }
	cur = *a; have = 1; note("set %d %d %u %u", a->rate, a->sound, a->points, a->frames); return 0;
}
int MI_AO_Enable(int dev) { (void)dev; enabled = 1; note("enable"); return 0; }
int MI_AO_Disable(int dev) { (void)dev; enabled = 0; note("disable"); return 0; }
int MI_AO_EnableChn(int dev, int chn) { (void)dev; (void)chn; note("enable chn"); return 0; }
int MI_AO_DisableChn(int dev, int chn) { (void)dev; (void)chn; note("disable chn"); return 0; }
int MI_AO_SendFrame(int dev, int chn, Frame *f, int wait) {
	(void)dev; (void)chn; (void)wait;
	short *s = f->addr[0]; unsigned long long sum = 0;
	for (unsigned i = 0; i < f->len / 2; i++) sum += (unsigned)(s[i] < 0 ? -s[i] : s[i]);
	sent_frames++; sent_sum += sum;
	if (f->addr[1]) note("send second buffer set");
	if (sent_frames <= 3 || sent_frames % 50 == 0) note("send #%llu len %u bits %d sound %d sum %llu", sent_frames, f->len, f->bits, f->sound, sum);
	return 0;
}
int MI_AO_QueryChnStat(int dev, int chn, State *st) { (void)dev; (void)chn; st->total = 65536; st->free = 65536; st->busy = 0; return 0; }

__attribute__((destructor)) static void report(void) { note("total sends %llu", sent_frames); }
