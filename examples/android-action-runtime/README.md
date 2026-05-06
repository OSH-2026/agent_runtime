# Android Action Runtime Example

This example launches the Kotlin action runtime as a foreground service and keeps it running in the background.

## Run

1) Open this folder in Android Studio.
2) Build and run the app on a device or emulator (API 34).
3) Accept the notifications permission when prompted.

The app starts `ActionRuntimeService` which hosts the gRPC server on port 8080.

## Notes

- The runtime module is linked via a local Gradle module reference to `kotlin/kotlin-actions-runtime`.
- Foreground service requires a notification on API 26+ and permission on API 33+.
