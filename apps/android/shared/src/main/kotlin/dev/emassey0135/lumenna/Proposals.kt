package dev.emassey0135.lumenna

import androidx.compose.runtime.staticCompositionLocalOf

/**
 * The moment a proposal is made against, `YYYY-MM-DDTHH:MM`: none in the app, which plans from
 * now. A test gives one, so what is proposed does not depend on the hour it runs.
 */
val LocalPlannerNow = staticCompositionLocalOf<String?> { null }
