// This exercises the same helper used by host.m without requiring UIKit.
#include "geometry.h"
#include <assert.h>
#include <stdio.h>

int main(void) {
    GpuiIosRect viewport = {0, 30, 400, 750}, keyboard = {0, 600, 400, 300};
    assert(gpui_ios_bottom_occlusion(viewport, keyboard) == 180);
    viewport.height = 570;
    assert(gpui_ios_bottom_occlusion(viewport, keyboard) == 0);
    viewport = (GpuiIosRect){10, 20, 800, 700};
    assert(gpui_ios_bottom_occlusion(viewport, (GpuiIosRect){100, 200, 300, 200}) == 0);
    assert(gpui_ios_bottom_occlusion(viewport, (GpuiIosRect){11, 600, 799, 200}) == 0);
    assert(gpui_ios_bottom_occlusion(viewport, (GpuiIosRect){10, 600, 800, 200}) == 120);
    assert(gpui_ios_bottom_occlusion(viewport, (GpuiIosRect){0, 0, 1000, 1000}) == 700);
    puts("iOS viewport occlusion: 6 assertions passed");
}
