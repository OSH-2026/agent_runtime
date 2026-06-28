# ActionFlow Action Catalog for LLM

本文件用于约束 LLM 生成可被当前 ActionFlow loader 和 Android Action Runtime
执行的 YAML。字段名区分大小写。`?` 表示可选字段，括号中为默认值。

## YAML 骨架

```yaml
version: 1
id: unique-workflow-id
steps:
  - id: device
    action: device_info
    inputs:
      includeHardware: true
```

在字符串中使用 `${step_id}` 引用上游输出并自动建立依赖。当前解析器不支持字段提取：
`${location.latitude}` 与 `${location}` 都会替换为整个 `location` JSON 字符串。

## 内置 Action（39）

### 设备与状态

```text
device_info({includeHardware?: bool=true}) -> DeviceInfoOutput
system_info({includeStorage?: bool=true}) -> SystemInfoOutput
network_status({includeDetails?: bool=true}) -> NetworkStatusOutput
power_status({includeDetails?: bool=true}) -> PowerStatusOutput
storage_info({includeExternal?: bool=true}) -> StorageInfoOutput
get_location({allowStaleMs?: long=600000}) -> LocationOutput
foreground_app({lookbackMs?: long=300000}) -> ForegroundAppOutput
check_permissions({permissions: string[]}) -> {granted: map<string,bool>}
```

### 系统控制与应用

```text
set_volume({level: int, stream?: "music"|"alarm"|"ring"|"notification"|"call"="music", showUi?: bool=false}) -> {stream:string, level:int}
set_silent_mode({mode?: "silent"|"vibrate"|"normal"="silent"}) -> {mode:string}
wifi_toggle({enabled: bool}) -> {enabled:bool}
bluetooth_toggle({enabled: bool}) -> {enabled:bool}
list_installed_apps({includeSystemApps?: bool=false}) -> {apps:InstalledApp[]}
launch_app({packageName: string}) -> LaunchResult
```

### 时钟、通信与个人数据

```text
set_alarm({hour:int, minutes:int, message?:string="Action Runtime alarm", skipUi?:bool=true}) -> LaunchResult
set_timer({lengthSeconds:int, message?:string="Action Runtime timer", skipUi?:bool=true}) -> LaunchResult
list_alarms({unused?:bool=true}) -> LaunchResult
read_sms({box?:"inbox"|"sent"|"draft"|"outbox"|"all"="inbox", limit?:int=50}) -> {messages:SmsMessage[]}
send_sms({address:string, body:string}) -> {sent:bool}
read_call_log({limit?:int=50}) -> {calls:CallLogItem[]}
place_call({phoneNumber:string}) -> {placed:bool}
search_contacts({query:string, limit?:int=20}) -> {results:ContactResult[]}
list_notifications({includeOngoing?:bool=true}) -> {notifications:NotificationEntry[]}
```

### 剪贴板与媒体

```text
clipboard_copy({text:string, label?:string="action_runtime"}) -> {copied:bool}
clipboard_read({unused?:bool=true}) -> {text:string, hasText:bool}
media_play_pause({action?:"play"|"pause"|"toggle"="toggle"}) -> {handled:bool}
media_now_playing({unused?:bool=true}) -> NowPlayingOutput
screenshot({timeoutMs?:long=3000}) -> {path:string, width:int, height:int}
screen_record({durationSeconds?:int=10, withAudio?:bool=false}) -> {path:string, durationSeconds:int}
take_photo({lens?:"back"|"front"="back"}) -> {path:string, width:int, height:int}
record_video({durationSeconds?:int=10, lens?:"back"|"front"="back", withAudio?:bool=false}) -> {path:string, durationSeconds:int}
record_audio({durationSeconds?:int=10}) -> {path:string, durationSeconds:int}
```

### 文件、网络与日历

```text
open_webpage({url:string, timeoutMs?:long=10000}) -> {finalUrl:string, title:string}
select_file({mimeType?:string="*/*"}) -> {uri:string, resultCode:int}
search_files({query:string, limit?:int=50}) -> {files:FileSearchItem[]}
read_file({path:string}) -> {content:string}
http_call({url:string}) -> {body:string, status:int}
list_calendar_events({startTimeMs:long, endTimeMs:long, limit?:int=50}) -> {events:CalendarEvent[]}
create_calendar_event({title:string, beginTimeMs:long, endTimeMs:long, description?:string="", location?:string="", calendarId?:long=null, timeZone?:string="UTC"}) -> {eventId:long, created:bool}
```

## Intent Action（20）

Intent Action 可能拉起系统 Activity、第三方应用或交互式选择器。

```text
intent_set_alarm({hour:int, minutes:int, message?:string="Action Runtime alarm", skipUi?:bool=true}) -> LaunchResult
intent_set_timer({lengthSeconds:int, message?:string="Action Runtime timer", skipUi?:bool=true}) -> LaunchResult
intent_show_alarms({unused?:bool=true}) -> LaunchResult
intent_insert_calendar({title:string, beginTimeMs:long, endTimeMs:long, description?:string="", location?:string="", calendarId?:long=null, timeZone?:string="UTC"}) -> {eventId:long, created:bool}
intent_capture_return({unused?:bool=true}) -> IntentActivityResult
intent_camera_still({lens?:"back"|"front"="back"}) -> {path:string, width:int, height:int}
intent_camera_video({durationSeconds?:int=10, lens?:"back"|"front"="back", withAudio?:bool=false}) -> {path:string, durationSeconds:int}
intent_pick_contact({unused?:bool=true}) -> IntentActivityResult
intent_pick_contact_data({mimeType?:string="vnd.android.cursor.item/phone_v2"}) -> IntentActivityResult
intent_view_contact({contactUri:string}) -> LaunchResult
intent_edit_contact({contactUri:string}) -> LaunchResult
intent_insert_contact({unused?:bool=true}) -> LaunchResult
intent_compose_email({to:string, subject?:string="", body?:string=""}) -> LaunchResult
intent_get_content({mimeType?:string="*/*"}) -> IntentActivityResult
intent_open_document({mimeType?:string="*/*"}) -> IntentActivityResult
intent_call_car({unused?:bool=true}) -> LaunchResult
intent_show_map({geoUri:string}) -> LaunchResult
intent_play_media({contentUri:string}) -> LaunchResult
intent_play_search({query:string, mediaFocus?:string="vnd.android.cursor.item/*"}) -> LaunchResult
intent_create_note({title?:string="", text?:string=""}) -> LaunchResult
```

## Tauri 本地 Action（3）

这些 action 只由当前 Tauri workflow demo 本地提供，不属于 Kotlin Android Runtime。

```text
text({value:string}) -> plain text
uppercase({text:string}) -> uppercase plain text
subagent({prompt:string}) -> plain text
```

## 输出结构

```text
LaunchResult = {launched:bool, resolvedPackage?:string|null, message:string}
IntentActivityResult = {launched:bool, resultCode:int, dataUri?:string|null, resolvedPackage?:string|null, message:string}
DeviceInfoOutput = {brand,model,manufacturer,device,product,sdkInt,release,securityPatch,supportedAbis,screenWidthPx,screenHeightPx,densityDpi,locale,timeZone}
SystemInfoOutput = {brand,model,sdkInt,release,uptimeMs,screenWidthPx,screenHeightPx,densityDpi,internalTotalBytes,internalAvailableBytes}
NetworkStatusOutput = {connected,transports,metered,downstreamKbps,upstreamKbps}
PowerStatusOutput = {batteryPercent,charging,powerSaveMode,plugged}
StorageInfoOutput = {internalTotalBytes,internalAvailableBytes,externalTotalBytes?,externalAvailableBytes?}
LocationOutput = {latitude,longitude,accuracyMeters,provider,timestampMs}
ForegroundAppOutput = {packageName,lastTimeUsed,available}
NowPlayingOutput = {title,artist,album,packageName,isPlaying}
InstalledApp = {packageName,label,isSystem}
SmsMessage = {id,address,body,dateMs,read,type}
CallLogItem = {id,number,type,dateMs,durationSeconds}
ContactResult = {contactId,displayName,phoneNumber}
NotificationEntry = {key,packageName,title,text,postTimeMs,isOngoing}
FileSearchItem = {id,displayName,sizeBytes,mimeType}
CalendarEvent = {id,title,description,location,beginTimeMs,endTimeMs}
```

## 生成规则

1. 只使用本目录列出的 action 和字段，不猜测 action 名或输入字段。
2. 每个 step 的 `id` 必须唯一，只使用字母、数字、下划线或短横线。
3. 无依赖的只读节点应并行放置；通过 `${step_id}` 建立汇合依赖。
4. 时间均使用 Unix epoch 毫秒；时区使用 IANA 名称，如 `Asia/Shanghai`。
5. 不要生成 `output`、`outputContract`、`outputs`、`policy`、`sideEffect`、`retryBudget` 或 `timeoutMs`。风险、确认、
   超时和重试策略由可信 Action Registry 元数据提供。
6. 不要把整个 `${step_id}` JSON 填入要求数字、布尔值或数组的字段。当前引用替换
    只生成字符串，最适合用于 `text`、`body`、`subject` 等字符串字段。
7. `http_call` 当前只支持 GET，输入只有 `url`。
8. 本 catalog 只约束 workflow YAML 内容；外层是否使用 Markdown 围栏由调用方协议决定。

## 建议的 LLM 指令

```text
根据用户目标生成 ActionFlow YAML。严格使用 action catalog 中存在的 action 和输入字段。
优先并行执行互不依赖的只读步骤，用 ${step_id} 创建依赖。不要生成任何执行策略字段，
这些策略由 Action Registry 提供。不要使用字段级引用，因为当前运行时只会注入整个上游 JSON 字符串。
只在 YAML 内容中放 workflow，不要在 YAML 内添加解释文字。
```
