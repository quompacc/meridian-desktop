#include <X11/Xlib.h>

#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

enum {
    POLL_NS = 1000000,
    PHASE_SECONDS = 4,
};

static void sleep_poll(void) {
    struct timespec delay = { .tv_sec = 0, .tv_nsec = POLL_NS };
    while (nanosleep(&delay, &delay) == -1 && errno == EINTR) {
    }
}

static int64_t monotonic_ns(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (int64_t)now.tv_sec * 1000000000LL + now.tv_nsec;
}

static int phase_active(int64_t started_at) {
    return monotonic_ns() - started_at < (int64_t)PHASE_SECONDS * 1000000000LL;
}

static void marker(const char *phase) {
    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);
    printf("%lld.%03ld %s\n", (long long)now.tv_sec, now.tv_nsec / 1000000, phase);
    fflush(stdout);
}

static int triangle(int step, int period, int amplitude) {
    int position = step % period;
    int half = period / 2;
    if (position > half) {
        position = period - position;
    }
    return position * amplitude / half;
}

int main(void) {
    Display *display = XOpenDisplay(NULL);
    if (display == NULL) {
        fputs("cannot open DISPLAY\n", stderr);
        return 1;
    }

    int screen = DefaultScreen(display);
    Window root = RootWindow(display, screen);
    Window window = XCreateSimpleWindow(
        display, root, 100, 100, 720, 480, 0,
        BlackPixel(display, screen), WhitePixel(display, screen));
    XStoreName(display, window, "Meridian X11 motion benchmark");
    XMapWindow(display, window);
    XSync(display, False);

    marker("mapped-idle-start");
    sleep(2);

    marker("move-start");
    int step = 0;
    int64_t phase_started_at = monotonic_ns();
    while (phase_active(phase_started_at)) {
        int x = 80 + triangle(step, 120, 900);
        int y = 80 + triangle(step + 30, 120, 300);
        XMoveWindow(display, window, x, y);
        XFlush(display);
        ++step;
        sleep_poll();
    }
    XSync(display, False);
    printf("move-updates=%d\n", step);
    marker("move-end");

    marker("resize-start");
    step = 0;
    phase_started_at = monotonic_ns();
    while (phase_active(phase_started_at)) {
        unsigned int width = 480 + (unsigned int)triangle(step, 120, 600);
        unsigned int height = 320 + (unsigned int)triangle(step + 30, 120, 360);
        XResizeWindow(display, window, width, height);
        XFlush(display);
        ++step;
        sleep_poll();
    }
    XSync(display, False);
    printf("resize-updates=%d\n", step);
    marker("resize-end");

    marker("final-idle-start");
    sleep(2);
    marker("done");
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
