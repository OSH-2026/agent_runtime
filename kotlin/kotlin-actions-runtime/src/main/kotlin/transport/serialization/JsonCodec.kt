package transport.serialization

import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.Json

class JsonCodec(
    private val json: Json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    },
) : Codec {
    override fun <T> decode(bytes: ByteArray, serializer: KSerializer<T>): T {
        val text = bytes.toString(Charsets.UTF_8)
        return json.decodeFromString(serializer, text)
    }

    override fun <T> encode(value: T, serializer: KSerializer<T>): ByteArray {
        val text = json.encodeToString(serializer, value)
        return text.toByteArray(Charsets.UTF_8)
    }
}
