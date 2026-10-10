package io.github.emassey0135.lumenna.wear

import io.github.emassey0135.lumenna.LinkService

/** What the watch is to other devices: a Wear OS watch. */
const val PLATFORM = "wearos"

/** The watch's side of its own link to the phone, answered when the phone opens one. */
class WatchLinkService : LinkService() {
    override val platform = PLATFORM
}
