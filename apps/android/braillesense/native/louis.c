// The JNI over liblouis: text to cells with the mappings between them, and cells back to text,
// each in dot patterns, never the tables' ASCII. See Louis.kt.
#include <jni.h>
#include <stdlib.h>
#include <string.h>
#include "liblouis.h"

// How much bigger than its input a translation may come out: a contraction-free table with
// capital and number signs can double it, and more is refused rather than cut.
#define GROWTH 4

JNIEXPORT void JNICALL
Java_com_extra_han_lumenna_device_Louis_setDataPath(JNIEnv *env, jobject self, jstring path) {
    const char *chars = (*env)->GetStringUTFChars(env, path, NULL);
    lou_setDataPath(chars);
    (*env)->ReleaseStringUTFChars(env, path, chars);
}

// [cells] (a byte per cell, bit 0 dot 1), [outputPos] (for each UTF-16 unit, its cell) and
// [inputPos] (for each cell, the unit it came from), as an Object[3]; null if the table refused.
JNIEXPORT jobjectArray JNICALL
Java_com_extra_han_lumenna_device_Louis_translate(JNIEnv *env, jobject self, jstring tables,
                                                  jstring text, jint cursor) {
    const char *table = (*env)->GetStringUTFChars(env, tables, NULL);
    jsize length = (*env)->GetStringLength(env, text);
    const jchar *chars = (*env)->GetStringChars(env, text, NULL);
    int inLength = length;
    int outLength = length * GROWTH + 16;
    widechar *out = malloc(sizeof(widechar) * outLength);
    int *outputPos = malloc(sizeof(int) * (length + 1));
    int *inputPos = malloc(sizeof(int) * outLength);
    int cursorPos = cursor;
    int ok = lou_translate(table, (const widechar *)chars, &inLength, out, &outLength, NULL, NULL,
                           outputPos, inputPos, cursor >= 0 ? &cursorPos : NULL,
                           dotsIO | (cursor >= 0 ? compbrlAtCursor : 0));
    (*env)->ReleaseStringChars(env, text, chars);
    (*env)->ReleaseStringUTFChars(env, tables, table);
    jobjectArray result = NULL;
    if (ok) {
        jbyteArray cells = (*env)->NewByteArray(env, outLength);
        jbyte *bytes = (*env)->GetByteArrayElements(env, cells, NULL);
        for (int i = 0; i < outLength; i++) bytes[i] = (jbyte)(out[i] & 0xff);
        (*env)->ReleaseByteArrayElements(env, cells, bytes, 0);
        jintArray output = (*env)->NewIntArray(env, inLength);
        (*env)->SetIntArrayRegion(env, output, 0, inLength, outputPos);
        jintArray input = (*env)->NewIntArray(env, outLength);
        (*env)->SetIntArrayRegion(env, input, 0, outLength, inputPos);
        result = (*env)->NewObjectArray(env, 3, (*env)->FindClass(env, "java/lang/Object"), NULL);
        (*env)->SetObjectArrayElement(env, result, 0, cells);
        (*env)->SetObjectArrayElement(env, result, 1, output);
        (*env)->SetObjectArrayElement(env, result, 2, input);
    }
    free(out);
    free(outputPos);
    free(inputPos);
    return result;
}

// The text [cells] (a byte per cell) read back, or null if the table refused.
JNIEXPORT jstring JNICALL
Java_com_extra_han_lumenna_device_Louis_backTranslate(JNIEnv *env, jobject self, jstring tables,
                                                      jbyteArray cells) {
    const char *table = (*env)->GetStringUTFChars(env, tables, NULL);
    jsize length = (*env)->GetArrayLength(env, cells);
    jbyte *bytes = (*env)->GetByteArrayElements(env, cells, NULL);
    widechar *in = malloc(sizeof(widechar) * (length + 1));
    for (int i = 0; i < length; i++) in[i] = (widechar)(LOU_DOTS | (bytes[i] & 0xff));
    (*env)->ReleaseByteArrayElements(env, cells, bytes, JNI_ABORT);
    int inLength = length;
    int outLength = length * GROWTH * 2 + 16;
    widechar *out = malloc(sizeof(widechar) * outLength);
    int ok = lou_backTranslate(table, in, &inLength, out, &outLength, NULL, NULL, NULL, NULL, NULL,
                               dotsIO);
    (*env)->ReleaseStringUTFChars(env, tables, table);
    jstring result = ok ? (*env)->NewString(env, (const jchar *)out, outLength) : NULL;
    free(in);
    free(out);
    return result;
}
