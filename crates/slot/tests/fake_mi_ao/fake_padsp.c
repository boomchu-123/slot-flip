/* A stand-in for the console's preloaded libpadsp, for checking that slot's
 * vendor backend finds the calls the way a program that links the library does:
 * through everything the process has loaded, where a preload comes first.
 *
 * It defines the same four names fake_mi_ao.c defines, answers them as the audio
 * server would (success), and appends what it was asked to the file named by
 * FAKE_MI_AO_LOG with a "hook" prefix. Run with LD_PRELOAD pointing at it.
 * Every call it does not define still reaches fake_mi_ao.c.
 */
#include <stdio.h>
#include <stdlib.h>

static unsigned long long frames;

static void note(const char *what) {
	const char *path = getenv("FAKE_MI_AO_LOG");
	if (!path) return;
	FILE *f = fopen(path, "a"); if (!f) return;
	fprintf(f, "hook %s\n", what); fclose(f);
}

int MI_AO_SetPubAttr(int dev, void *a) { (void)dev; (void)a; note("set"); return 0; }
int MI_AO_Enable(int dev) { (void)dev; note("enable"); return 0; }
int MI_AO_EnableChn(int dev, int chn) { (void)dev; (void)chn; note("enable chn"); return 0; }
int MI_AO_SendFrame(int dev, int chn, void *f, int wait) {
	(void)dev; (void)chn; (void)f; (void)wait;
	if (++frames <= 3) note("send");
	return 0;
}
