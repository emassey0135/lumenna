package dev.emassey0135.lumenna

import dev.emassey0135.lumenna.app.PickerWords
import dev.emassey0135.lumenna.app.pickerWords
import dev.emassey0135.lumenna.app.sentenceCase

/** What a day or time picker beside a line says, the core's (`picker_words`), for the phone and the watch. */
val pickers: PickerWords by lazy {
    pickerWords().let { words ->
        PickerWords(
            sentenceCase(words.day), sentenceCase(words.time), sentenceCase(words.choose),
            sentenceCase(words.cancel), sentenceCase(words.hours), sentenceCase(words.minutes),
        )
    }
}
