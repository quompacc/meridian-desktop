#include <X11/Xlib.h>

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>

enum {
    STEP_NS = 16666667,
    MOVE_STEPS = 240,
    RESIZE_STEPS = 240,
};

static void sleep_step(void) {
    struct timespec delay = { .tv_sec = 0, .tv_nsec = STEP_NS };
    while (nanosleep(&delay, &delay) == -1 && errno == EINTR) {
    }
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
    for (int step = 0; step < MOVE_STEPS; ++step) {
        int x = 80 + triangle(step, 120, 900);
        int y = 80 + triangle(step + 30, 120, 300);
        XMoveWindow(display, window, x, y);
        XFlush(display);
        sleep_step();
    }
    XSync(display, False);
    marker("move-end");

    marker("resize-start");
    for (int step = 0; step < RESIZE_STEPS; ++step) {
        unsigned int width = 480 + (unsigned int)triangle(step, 120, 600);
        unsigned int height = 320 + (unsigned int)triangle(step + 30, 120, 360);
        XResizeWindow(display, window, width, height);
        XFlush(display);
        sleep_step();
    }
    XSync(display, False);
    marker("resize-end");

    marker("final-idle-start");
    sleep(2);
    marker("done");
    XDestroyWindow(display, window);
    XCloseDisplay(display);
    return 0;
}
