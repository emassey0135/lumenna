// Clean-room AIDL reconstruction of the input-service callback.
// Descriptor must remain "com.jawon.han.input.IHanBrailleInputServiceCallback".
//
// onResponse is the event channel for gestures and some system keys. Observed
// usage: arg1 is an event code (e.g. 2037 = sticky-key state change), arg2 a
// value/key, data a string payload. Return true if you consumed the event.
package com.extra.han.lumenna.han;

@Descriptor(value="com.jawon.han.input.IHanBrailleInputServiceCallback")
interface IHanBrailleInputServiceCallback {
    boolean onResponse(int code, int value, String data);                       // 1
}
