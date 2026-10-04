import AppKit

// No storyboard or nib: the menus, windows and status item are built in code, where each
// one's accessibility can be read beside what it does.
let application = NSApplication.shared
let delegate = AppDelegate()
application.delegate = delegate
_ = NSApplicationMain(CommandLine.argc, CommandLine.unsafeArgv)
