package com.extra.han.lumenna.han;

import android.os.Parcel;
import android.os.Parcelable;

/**
 * Clean-room reimplementation of the HanOutput data object exchanged with the
 * BrailleSense output service. This is an original implementation written to a
 * reverse-engineered wire format; it carries no code from the vendor SDK.
 *
 * One HanOutput independently carries three strings and a set of per-utterance
 * overrides:
 *   lcd     - text for the visual LCD line
 *   tts     - text to speak
 *   braille - text for the braille cells (the vendor apps place ALREADY-TRANSLATED
 *             braille here, produced client-side; pass plain text only if you have
 *             confirmed the service translates it for you)
 *   debug   - free/debug string (vendor apps also use it to pass option tags)
 *
 * WIRE FORMAT (must not be reordered) - four strings then seven ints:
 *   lcd, tts, braille, debug,
 *   cursorPos, cursorLen, volume, pitch, speed, isEnglish, lcdPos
 */
public class HanOutput implements Parcelable {

    // Sentinel used by the service to mean "no per-utterance override; use the
    // current global voice setting". Observed default for volume/pitch/speed and
    // the cursor/LCD position fields.
    public static final int DEFAULT = -1;

    // Observed default for the isEnglish/engine-select field.
    public static final int ENGINE_DEFAULT = 10000;

    public String lcd;
    public String tts;
    public String braille;
    public String debug;

    public int cursorPos = -1;
    public int cursorLen = 0;
    public int volume = -1;
    public int pitch = -1;
    public int speed = -1;
    public int isEnglish = ENGINE_DEFAULT;
    public int lcdPos = -1;

    public HanOutput() {
    }

    /** Convenience: same text to LCD, speech and braille. */
    public HanOutput(String text) {
        this.lcd = text;
        this.tts = text;
        this.braille = text;
        this.debug = "";
    }

    public HanOutput(String lcd, String tts, String braille, String debug) {
        this.lcd = lcd;
        this.tts = tts;
        this.braille = braille;
        this.debug = debug;
    }

    protected HanOutput(Parcel in) {
        lcd = in.readString();
        tts = in.readString();
        braille = in.readString();
        debug = in.readString();
        cursorPos = in.readInt();
        cursorLen = in.readInt();
        volume = in.readInt();
        pitch = in.readInt();
        speed = in.readInt();
        isEnglish = in.readInt();
        lcdPos = in.readInt();
    }

    @Override
    public void writeToParcel(Parcel out, int flags) {
        out.writeString(lcd);
        out.writeString(tts);
        out.writeString(braille);
        out.writeString(debug);
        out.writeInt(cursorPos);
        out.writeInt(cursorLen);
        out.writeInt(volume);
        out.writeInt(pitch);
        out.writeInt(speed);
        out.writeInt(isEnglish);
        out.writeInt(lcdPos);
    }

    @Override
    public int describeContents() {
        return 0;
    }

    public static final Creator<HanOutput> CREATOR = new Creator<HanOutput>() {
        @Override
        public HanOutput createFromParcel(Parcel in) {
            return new HanOutput(in);
        }

        @Override
        public HanOutput[] newArray(int size) {
            return new HanOutput[size];
        }
    };
}
