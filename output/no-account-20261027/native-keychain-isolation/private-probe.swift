import Foundation
import Security
SecKeychainSetUserInteractionAllowed(false)
var beforeDefault: SecKeychain?
let beforeDefaultStatus = SecKeychainCopyDefault(&beforeDefault)
var beforeList: CFArray?
let beforeListStatus = SecKeychainCopySearchList(&beforeList)
var keychain: SecKeychain?
let password = Array("synthetic-private-keychain-password".utf8)
let path = CommandLine.arguments[1]
let created = password.withUnsafeBytes { bytes in
    path.withCString { name in SecKeychainCreate(name, UInt32(bytes.count), bytes.baseAddress, false, nil, &keychain) }
}
var add: OSStatus = -1
var read: OSStatus = -1
var removed: OSStatus = -1
var matched = false
if created == errSecSuccess, let keychain {
    let secret = Data("synthetic-private-probe-only".utf8)
    let attributes: [String: Any] = [kSecClass as String:kSecClassGenericPassword, kSecAttrService as String:"com.claude-switcher.native.probe", kSecAttrAccount as String:"synthetic", kSecUseKeychain as String:keychain, kSecValueData as String:secret]
    add = SecItemAdd(attributes as CFDictionary, nil)
    var value: CFTypeRef?
    let query: [String:Any] = [kSecClass as String:kSecClassGenericPassword, kSecAttrService as String:"com.claude-switcher.native.probe", kSecAttrAccount as String:"synthetic", kSecMatchSearchList as String:[keychain], kSecReturnData as String:true]
    read = SecItemCopyMatching(query as CFDictionary, &value)
    matched = (value as? Data) == secret
    removed = SecKeychainDelete(keychain)
}
var afterDefault: SecKeychain?
let afterDefaultStatus = SecKeychainCopyDefault(&afterDefault)
var afterList: CFArray?
let afterListStatus = SecKeychainCopySearchList(&afterList)
let defaultSame = beforeDefaultStatus == afterDefaultStatus && (beforeDefault == nil ? afterDefault == nil : afterDefault != nil && CFEqual(beforeDefault, afterDefault))
let listSame = beforeListStatus == afterListStatus && (beforeList == nil ? afterList == nil : afterList != nil && CFEqual(beforeList, afterList))
let result:[String:Any] = ["create":created,"add":add,"read":read,"matched":matched,"delete":removed,"default_unchanged":defaultSame,"search_list_unchanged":listSame,"default_status_before":beforeDefaultStatus,"search_list_status_before":beforeListStatus]
let data = try JSONSerialization.data(withJSONObject:result, options:[.sortedKeys])
print(String(decoding:data,as:UTF8.self))
exit(created == 0 && add == 0 && read == 0 && matched && removed == 0 && defaultSame && listSame ? 0 : 1)
