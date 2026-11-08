// Clean-room AIDL reconstruction of the BrailleSense output service interface.
//
// This file is an INTERFACE DEFINITION reconstructed from the public binder ABI
// of com.jawon.han.accessibility/com.jawon.han.output.HanOutputService so that an
// independent app can interoperate with it. It contains no implementation code.
//
// WIRE-COMPATIBILITY RULES (do not reorder):
//   * The binder descriptor must stay exactly "com.jawon.han.output.IHanOutputService",
//     which is why this interface lives in package com.jawon.han.output.
//   * Transaction codes are positional: AIDL assigns FIRST_CALL_TRANSACTION + N in
//     source order. The method ORDER below reproduces the observed codes 1..38.
//     Parameter TYPES must match; parameter NAMES are ours and do not affect the wire.
package com.extra.han.lumenna.han;

import com.extra.han.lumenna.han.HanOutput;
import com.extra.han.lumenna.han.IHanOutputServiceCallback;

@Descriptor(value="com.jawon.han.output.IHanOutputService")
interface IHanOutputService {
    boolean isSpeaking();                                                       // 1
    int displayAndPlayTTS(int mode, in HanOutput output,                        // 2
                          boolean flag1, boolean flag2, boolean flag3,
                          int volume, int speed, int pitch, int extra,
                          IHanOutputServiceCallback callback);
    int stopTTS();                                                              // 3
    boolean brailleCursorPos(int pos);                                          // 4
    boolean brailleCursorRange(int start, int end);                            // 5
    boolean brailleCursorType(int type);                                        // 6
    void brailleCursorBlinkOn(boolean on);                                      // 7
    int getBrailleCellLine();                                                   // 8
    int getBrailleTotalCellLine();                                              // 9
    boolean setCurrentCellLine(int line);                                       // 10
    void displayBraille(String text);                                           // 11
    void displayBraille2(String text, int arg1, int arg2);                      // 12
    void displayBrailleWordWrap(String text, boolean wordWrap);                 // 13
    void displayRawBraille(in byte[] dotPatterns);                              // 14
    void displayLcd(String text);                                              // 15
    void displayOutputArray(in HanOutput[] outputs,                            // 16
                            IHanOutputServiceCallback callback);
    int getVoiceVolume();                                                       // 17
    int setVoiceVolume(int volume, boolean apply);                             // 18
    int getVoiceSpeed();                                                        // 19
    int setVoiceSpeed(int speed, boolean apply);                              // 20
    int getVoicePitch();                                                        // 21
    int setVoicePitch(int pitch, boolean apply);                              // 22
    void register(IBinder clientToken);                                         // 23
    void unregister(IBinder clientToken);                                       // 24
    void setTerminalScreenReaderCell(boolean on);                              // 25
    int getTTSStatus();                                                         // 26
    void setActiveOutputService(boolean active);                               // 27
    boolean getActiveOutputService();                                           // 28
    boolean isTtsSetupDone();                                                   // 29
    int getTtsEngineType();                                                     // 30
    String getLastMessage();                                                    // 31
    void skipSaveBraille();                                                     // 32
    void disableSkipAnotherMessage();                                           // 33
    void outputLastBrailleMessage();                                            // 34
    String getTopPackageClassName();                                            // 35
    String getTopPackageName();                                                 // 36
    void setSkipLastBrailleMessage(boolean skip);                              // 37
    void enableSkipAnotherMessage();                                            // 38
}
