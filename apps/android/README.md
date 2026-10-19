# Lumenna on Android

Jetpack Compose over the Crux app (`crates/ffi`), through BoltFFI's Kotlin bindings. Four
tabs as on the iPhone — Today, Tasks, Browse, Settings — each with its own back stack, and
every rule the core's: the screens draw what the app shows and send what the person did.

## Building

Needs Android Studio's SDK (platform 37), an NDK, Rust's Android targets and BoltFFI's CLI
at the version the crates use:

```
sdkmanager "ndk;30.0.16248370"            # from the SDK's cmdline-tools
rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi
cargo install boltffi_cli --version 0.31.1 --locked
```

`LUMENNA_ANDROID_ABIS=arm64-v8a` builds for one ABI only, for a quicker build onto an Arm
emulator or phone.

Then, with Android Studio's JDK:

```
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
./gradlew :app:assembleDebug                # build
./gradlew :app:installDebug                 # onto a running emulator or phone
./gradlew :app:connectedDebugAndroidTest    # the instrumented tests
```

Gradle's `buildCore` task runs `build-core.sh` before every build: `boltffi pack android`
builds the core for each ABI and writes its Kotlin, both into `shared/build/generated` —
nothing generated is committed. Cargo does nothing when nothing changed. Android Studio opens this directory as an
ordinary Gradle project.

## Accessibility

- **A row is one TalkBack stop.** Its title is the description and everything else its state
  ("done, due tomorrow, priority 1"), worded as the iPhone words it (`AppRowSpeech`). Depth is
  said where it changes ("level 2") and never left to indentation.
- **Every action is a custom accessibility action**, named for what it does ("Mark Done",
  "Start Timer"), and the same actions are on a long press.
- **Lists are collections**, so TalkBack says "3 of 12"; each screen is a pane with a heading.
- **Focus after a change is chosen**, as on the iPhone: the same row if it is still listed,
  else whichever now holds its place (`RowFocus`). TalkBack's focus is moved there
  explicitly — it does not follow input focus under touch — and only then is the change said.
- **What a change did is said by a snackbar**, a polite live region that stays as long as the
  person's accessibility timeout asks.
- **The tests run Google's accessibility checks on every interaction**
  (`enableAccessibilityChecks`), as the iPhone's run Apple's audit — which is how the 40dp
  buttons and radio rows were found and made 48dp. One test checks TalkBack's own focus after
  a change; it runs when TalkBack is on in the emulator and is skipped otherwise:

  ```
  adb shell settings put secure enabled_accessibility_services \
    com.google.android.marvin.talkback/com.google.android.marvin.talkback.TalkBackService
  adb shell settings put secure accessibility_enabled 1
  ```

## New Task from anywhere

Long-pressing the app icon offers New task (TalkBack lists it among the icon's actions), and a
Quick Settings tile does the same: the app opens on quick add, over the Tasks tab.

## Syncing in the background

While the app is in front it syncs continuously. When it is left, WorkManager runs one round
at once, so what was just edited reaches the other devices, and then one every 15 minutes or
so — Android's least, stretched when the phone dozes or the app is seldom opened. A round
reaches only the devices running at that moment, such as a Mac or `lum daemon`.

## Reminders

Android schedules them itself: the app hands AlarmManager the next two days' firings, as the
core words them, and sets them again whenever they may have moved. They come at their exact
time if Lumenna may set exact alarms (Settings, Reminders, Allow exact alarms, which opens
Android's Alarms and reminders page); otherwise a few minutes late, as Android allows. Each is
a notification on the Reminders channel with Dismiss and Snooze, which reach every device. The
watch gives its own, and the phone's are not passed on to it, so each comes once there.

## Not yet

Pairing on the local network is untested on Android, because the emulator sits behind its own
network address translation and multicast does not cross it: it needs a real phone. Discovery uses
`mdns-sd`, which hears multicast only while the app holds a multicast lock — held while it syncs
in front and while it pairs; it is not exclusive, and the socket shares port 5353 with every
other mDNS user. Pairing by code is the dependable way for now, as on the iPhone. The two pairing tests the iPhone has need a second
device and are not ported. Wear OS is its own module, not begun.
