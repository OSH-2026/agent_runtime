package transport.serialization

import kotlinx.serialization.KSerializer

interface Codec {
    fun <T> decode(bytes: ByteArray, serializer: KSerializer<T>): T
    fun <T> encode(value: T, serializer: KSerializer<T>): ByteArray
}
