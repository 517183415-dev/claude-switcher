#import <Foundation/Foundation.h>
#import <dispatch/dispatch.h>

extern void switcher_swiftui_bridge_fixture_start(void);

__attribute__((constructor)) static void start_bridge_fixture(void) {
    dispatch_async(dispatch_get_main_queue(), ^{
        switcher_swiftui_bridge_fixture_start();
    });
}
