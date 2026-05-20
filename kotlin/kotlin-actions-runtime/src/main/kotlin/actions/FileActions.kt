package actions

import android.content.Intent
import android.provider.MediaStore
import api.Action
import api.ActionContext
import error.ActionException
import error.ErrorCode
import kotlinx.serialization.Serializable
import runtime.IntentHostCoordinator
import util.PermissionUtils

@Serializable
data class SelectFileInput(
    val mimeType: String = "*/*",
)

@Serializable
data class SelectFileOutput(
    val uri: String,
    val resultCode: Int,
)

class SelectFileAction : Action<SelectFileInput, SelectFileOutput> {
    override suspend fun execute(input: SelectFileInput, ctx: ActionContext): SelectFileOutput {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = input.mimeType
        }
        val result = IntentHostCoordinator.startForResult(ctx.appContext, intent)
        return SelectFileOutput(
            uri = result.dataUri ?: "",
            resultCode = result.resultCode,
        )
    }
}

@Serializable
data class FileSearchInput(
    val query: String,
    val limit: Int = 50,
)

@Serializable
data class FileSearchItem(
    val id: Long,
    val displayName: String,
    val sizeBytes: Long,
    val mimeType: String,
)

@Serializable
data class FileSearchOutput(val files: List<FileSearchItem>)

class FileSearchAction : Action<FileSearchInput, FileSearchOutput> {
    override suspend fun execute(input: FileSearchInput, ctx: ActionContext): FileSearchOutput {
        val context = ctx.appContext
        val hasMediaPerm = PermissionUtils.anyGranted(
            context,
            listOf(
                android.Manifest.permission.READ_MEDIA_IMAGES,
                android.Manifest.permission.READ_MEDIA_VIDEO,
                android.Manifest.permission.READ_MEDIA_AUDIO,
                android.Manifest.permission.READ_EXTERNAL_STORAGE,
            ),
        )
        if (!hasMediaPerm) {
            throw ActionException(
                code = ErrorCode.PERMISSION,
                message = "Storage read permission required",
                retryable = false,
            )
        }
        val projection = arrayOf(
            MediaStore.Files.FileColumns._ID,
            MediaStore.Files.FileColumns.DISPLAY_NAME,
            MediaStore.Files.FileColumns.SIZE,
            MediaStore.Files.FileColumns.MIME_TYPE,
        )
        val selection = "${MediaStore.Files.FileColumns.DISPLAY_NAME} LIKE ?"
        val selectionArgs = arrayOf("%${input.query}%")
        val files = mutableListOf<FileSearchItem>()
        context.contentResolver.query(
            MediaStore.Files.getContentUri("external"),
            projection,
            selection,
            selectionArgs,
            MediaStore.Files.FileColumns.DATE_MODIFIED + " DESC",
        )?.use { cursor ->
            val idIndex = cursor.getColumnIndex(MediaStore.Files.FileColumns._ID)
            val nameIndex = cursor.getColumnIndex(MediaStore.Files.FileColumns.DISPLAY_NAME)
            val sizeIndex = cursor.getColumnIndex(MediaStore.Files.FileColumns.SIZE)
            val mimeIndex = cursor.getColumnIndex(MediaStore.Files.FileColumns.MIME_TYPE)
            while (cursor.moveToNext() && files.size < input.limit) {
                files.add(
                    FileSearchItem(
                        id = cursor.getLong(idIndex),
                        displayName = cursor.getString(nameIndex) ?: "",
                        sizeBytes = cursor.getLong(sizeIndex),
                        mimeType = cursor.getString(mimeIndex) ?: "",
                    ),
                )
            }
        }
        return FileSearchOutput(files)
    }
}
