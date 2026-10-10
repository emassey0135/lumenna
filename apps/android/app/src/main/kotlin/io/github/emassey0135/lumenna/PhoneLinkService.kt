package io.github.emassey0135.lumenna

/** The phone's side of a Wear OS watch's own link, answered when the watch opens one. */
class PhoneLinkService : LinkService() {
    override val platform = "android"
}
