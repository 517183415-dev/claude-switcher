import Foundation
import Security
SecKeychainSetUserInteractionAllowed(false)
var keychain: SecKeychain?
let status = SecKeychainCopyDefault(&keychain)
print("{\"SecKeychainCopyDefault\":\(status),\"interaction_disabled\":true}")
