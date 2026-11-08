// Clean-room AIDL reconstruction of the output-service callback interface.
// Descriptor must remain "com.jawon.han.output.IHanOutputServiceCallback".
// Single method at transaction code 1. The five longs are TTS event fields
// (status/progress markers); semantics to be confirmed empirically.
package com.extra.han.lumenna.han;

@Descriptor(value="com.jawon.han.output.IHanOutputServiceCallback")
interface IHanOutputServiceCallback {
    void onEventTTS(long arg1, long arg2, long arg3, long arg4, long arg5);     // 1
}
