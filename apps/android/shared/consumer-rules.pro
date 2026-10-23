# The core reaches its Kotlin bindings by name through JNI both ways: the `Native` object's
# external functions, and the callback objects it calls back into (`ShellCallbacks.turn`, the
# listener's and the locale's), which nothing in Kotlin calls and R8 would otherwise remove or
# rename. The records cross by BoltFFI's own encoding, read and written by the same classes.
-keep class dev.emassey0135.lumenna.app.** { *; }
