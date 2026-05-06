package transport.serialization

import kotlinx.serialization.KSerializer

class ProtoCodec : Codec {
    override fun <T> decode(bytes: ByteArray, serializer: KSerializer<T>): T {
        throw UnsupportedOperationException("ProtoCodec not implemented")
    }

    override fun <T> encode(value: T, serializer: KSerializer<T>): ByteArray {
        throw UnsupportedOperationException("ProtoCodec not implemented")
    }
}
