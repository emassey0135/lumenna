// Clean-room AIDL reconstruction of the BrailleSense braille-input / keyboard
// control service. Interface definition only; no vendor implementation code.
//
// IMPORTANT - this service is NOT Intent-bound. It is registered in the ROM's
// framework as a custom system service and obtained with
//     getSystemService("han_brailleinput")
// (siblings: "han_cellout", "han_lcdout", "han_beep"). See HanInputClient.java
// and the README for the third-party access caveats (classloader cast + hidden
// ServiceManager + per-method permission checks).
//
// This is a CONTROL / key-injection interface, not where an app passively
// receives typed text. Ordinary braille/QWERTY keystrokes still arrive as normal
// Android KeyEvents to your focused View. Gestures and some system-key events
// arrive through IHanBrailleInputServiceCallback.onResponse(...).
//
// Method ORDER reproduces observed transaction codes 1..38 - do not reorder.
// Types must match; parameter names here are ours. Vendor method names kept
// as-is (including the "Ingnore" spelling) only to ease cross-referencing; they
// do not affect the wire.
package com.extra.han.lumenna.han;

import com.extra.han.lumenna.han.IHanBrailleInputServiceCallback;

@Descriptor(value="com.jawon.han.input.IHanBrailleInputService")
interface IHanBrailleInputService {
    void injectKey(int keyCode, int metaState, int arg3);                       // 1
    void playMacro(String macro);                                               // 2
    void registerCallback(IHanBrailleInputServiceCallback callback);            // 3
    void unregisterCallback(IHanBrailleInputServiceCallback callback);          // 4
    void lockSystemKey();                                                       // 5
    void releaseSystemKey();                                                    // 6
    void lockHomeKey();                                                         // 7
    void unlockHomeKey();                                                       // 8
    void setAvailableSystemKey(in int[] keys);                                  // 9
    void eraseTTSStopFlag();                                                    // 10
    boolean checkTTSStopFlag();                                                 // 11
    void powerOffAppResponse();                                                 // 12
    void appLoadingComplete();                                                  // 13
    void setUpgradeStatus(boolean upgrading);                                   // 14
    boolean checkUpgradeStatus();                                               // 15
    int setAtlasFrequency(int frequency);                                       // 16
    int getLoadingCompleteStatus();                                             // 17
    void set6DotMode(boolean sixDot);                                           // 18
    boolean is6DotMode();                                                       // 19
    void setJpnInputMode(int mode);                                             // 20
    int getJpnInputMode();                                                      // 21
    void setJpnDialogState(int state);                                          // 22
    int getJpnDialogState();                                                    // 23
    int getMacroPeriod();                                                       // 24
    void setMacroPeriod(int period);                                            // 25
    void setMacroDelayInputMode(boolean delay);                                 // 26
    int updateScanChipFirmware(in byte[] firmware);                             // 27
    int setProductionMode(int mode);                                            // 28
    int getOneHandKeyValue();                                                   // 29
    int getStickyMetaState();                                                   // 30
    boolean isStickyKeyMode();                                                  // 31
    void enableStickyKeyMode(boolean enable);                                   // 32
    void turnOnStickyKey(int key, boolean on);                                  // 33
    void setTTSSyncMode(boolean sync);                                          // 34
    void setIngnoreQWERTYHotKey(boolean ignore);                               // 35
    boolean getIngnoreQWERTYHotKey();                                           // 36
    int getStickyKeyState(int key);                                            // 37
    void setStickyKeyState(int key, int state);                                // 38
}
