// Interop with Orbit's braille translator. Method order fixes transaction codes 1..3; only these are declared
// because only translateBrailleToText (code 3) is used. The service's descriptor matches this package+name.
package com.orbit.liblouisservice;

interface ILiblouisServiceInterface {
    boolean isInitialized();
    CharSequence translateTextToBraille(String text, int mode, String tableFileName);
    CharSequence translateBrailleToText(String text, int mode, String tableFileName);
}
