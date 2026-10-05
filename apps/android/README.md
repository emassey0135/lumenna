# Lumenna on Android

Jetpack Compose over the Rust core (§16.7), through UniFFI's Kotlin bindings and JNA. Four
tabs as on the iPhone — Today, Tasks, Browse, Settings — each with its own back stack, and
every operation the core's: the app words what it returns and decides nothing else.

## Building

Needs Android Studio's SDK (platform 37), an NDK, Rust's `aarch64-linux-android` target and
`cargo-ndk`:

```
sdkmanager "ndk;30.0.16248370"            # from the SDK's cmdline-tools
rustup target add aarch64-linux-android
cargo install cargo-ndk
```

Then, with Android Studio's JDK:

```
export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
./gradlew :app:assembleDebug                # build
./gradlew :app:installDebug                 # onto a running emulator or phone
./gradlew :app:connectedDebugAndroidTest    # the instrumented tests
```

Gradle's `buildCore` task runs `build-core.sh` before every build: the core for Arm64, and the
Kotlin bindings from a host build, both into `app/build/generated` — nothing generated is
committed. Cargo does nothing when nothing changed. Android Studio opens this directory as an
ordinary Gradle project.

## Accessibility

- **A row is one TalkBack stop.** Its title is the description and everything else its state
  ("done, due tomorrow, priority 1"), worded as the iPhone words it (`RowSpeech`). Depth is
  said where it changes ("level 2") and never left to indentation.
- **Every action is a custom accessibility action**, named for what it does ("Mark Done",
  "Start Timer"), and the same actions are on a long press.
- **Lists are collections**, so TalkBack says "3 of 12"; each screen is a pane with a heading.
- **What a change did is said by a snackbar**, a polite live region that stays as long as the
  person's accessibility timeout asks.
- **The tests run Google's accessibility checks on every interaction**
  (`enableAccessibilityChecks`), as the iPhone's run Apple's audit — which is how the 40dp
  buttons and radio rows were found and made 48dp.

## Not yet

Pairing on the local network is untested on Android: discovery uses `mdns-sd`, which hears
multicast only while the app holds a multicast lock (taken while pairing). Pairing by code is
the dependable way for now, as on the iPhone. The two pairing tests the iPhone has need a second
device and are not ported. Wear OS (§16.8) is its own module, not begun.
