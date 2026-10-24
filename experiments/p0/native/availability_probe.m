// Compile-only SDK declaration check. This translation unit is never executed.
#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import <ServiceManagement/ServiceManagement.h>
#include <xpc/xpc.h>

void check_declarations(xpc_connection_t peer, xpc_object_t message,
                        NSXPCConnection *connection) {
    int result = xpc_connection_set_peer_code_signing_requirement(peer, "anchor apple");
    SecCodeRef code = NULL;
    OSStatus status = SecCodeCreateWithXPCMessage(message, 0, &code);
    [connection setCodeSigningRequirement:@"anchor apple"];
    SMAppService *service = [SMAppService mainAppService];
    SMAppServiceStatus state = [service status];
    (void)result;
    (void)status;
    (void)state;
}
