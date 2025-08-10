# mania-rating-gui
A rating calcuator for osu!mania 6K mode.

![](svg/icon.svg)

# 使用方法：
打开程序后，程序会加载一段时间，包括读取本地osu的成绩，并绘制近30次成绩的卡片(Recent 30)。之后程序会显示这样的界面：

![主界面](/pics/main.jpg "主界面")

窗口的左上角的下拉框包含了所有本地osu!目录内有osu!mania 6K谱面游玩记录的玩家名称，以及Recent 30，通过选择玩家名称即可加载其最佳30个游玩记录(Best 30)并绘制卡片。卡片的样例如下所示：

![卡片](/ui/rating_example.png "卡片")

单个卡片的大小为1200\*350像素，左边是300\*300的谱面背景，右边的底层图层是背景经过了高斯模糊和暗化处理。在卡片的中间展示了谱面的标题、艺术家、创作者、难度名等基本信息，下方是BPM、时长、经过SR Reborn计算的难度星级、以及单点和长键的物量。在卡片右边是成绩相关信息，包括在游玩记录中的排名，玩家名称与游玩时间，使用的mod（只显示DT, HT, ScoreV2，不计入Random成绩），各个判定的数量，以及谱面的6K定级和游玩Rating。在判定占比圈的下方有两个百分数，绿色的是按照下方Rating算法部分计算的Rating Acc（彩310，黄300），白色的是游戏中实际的Acc。

界面内的成绩卡片会在右下角有红色的"-""按钮或绿色的"+"按钮。在点击红色按钮之后，对应的卡片将会从成绩列表移除，添加到备选列表。如果玩家的成绩还有剩余，如在开始移除一个之后，游玩的记录数>31，那么就会在成绩列表自动填充剩余的最好的成绩。

而进入到备选列表的卡片右下角的按钮会变为绿色按钮，在点击按钮之后，会移除成绩列表最差的一个成绩，并将这个成绩卡片重新放回成绩列表。点击左上角的"重置"按钮即可重新将成绩列表设置为最佳的30个记录。

通过加减按钮处理了不想要的记录（如谱面未上传到官网，或者与游玩记录内有相同的谱面）后，即可点击导出按钮，导出成绩列表的所有卡片，导出位置将会在窗口右上角提示。

![导出](/pics/exported.jpg "导出")

导出的样例如下所示：

![导出图像](/pics/SiFouR.png "导出图像")

在v0.2.0版本添加了复制功能，可以点击卡片左下角的灰色按钮，将单个卡片复制到剪切板，从而可以直接在QQ等聊天软件内粘贴发送。此外还增加了实验性的实时读取功能，在启动本程序打开实时读取模式后，开启osu!游戏，程序就会通过读取osu!内存自动读取并展示游玩后的成绩。

![实时模式](/pics/realtime.jpg "实时模式")

# 免责声明

1. 本程序对osu!stable安装目录下的 osu!.db 和 scores.db 进行分析，
    通过scores.db中游玩记录的谱面MD5找到Songs文件夹中对应的.osu谱面文件，
    筛选出模式为Mania 6K且有游玩记录的谱面。
    使用sunnyxxy的Star-Rebirth 20250415版本计算星级，
    使用sunnyxxy的Rating算法计算玩家表现。目前无法用于osu!lazer。

2. 由于osu!中的数据库使用明文储存，本程序没有任何反作弊手段，
    无法读取和验证玩家的Replay，仅读取存在的分数。
    且星级算法和Rating算法仍在早期开发阶段，对LN和高速等谱面SR测定仍然较高。
    此外可能计入重复的谱面，本程序所测数据仅供参考。

3. 本程序没有任何联网功能，不会读取玩家的个人隐私数据，
    也不会向互联网上传和下载任何内容。
    本程序只读取本地数据库，不会修改任何数据库内容。
    由于osu!.db内部格式经常修改，本程序适配版本为20250401版本的数据库，
    后续可能因版本变化导致无法运行。

4. 实时模式需要读取osu!.exe进程的内存，目前只提供了对stable客户端的支持。
    读取进程内存可能会被杀毒软件拦截，或面临被osu官方服务器或私服封禁的风险。
    本程序在运行过程中不会修改其他进程的内容，但仍建议谨慎使用。

# 使用的算法

## [sunnyxxy SR Reborn](https://github.com/sunnyxxy/Star-Rating-Rebirth)
使用2025/04/15版本。

## Rating计算
使用sunnyxxy osu!主页展示的[google表格](https://docs.google.com/spreadsheets/d/1orVFRc_dmCDaQaIEGi1vcjePcZ0od0qMriuUNJOQUO0/edit?pli=1&gid=777965813#gid=777965813)中的公式：

$$\text{Params: } \text{diff}'=\max(\text{diffConst}-3,0)$$

$$\begin{equation}\notag\text{rating}=f(\text{acc}, \text{diffConst})=\begin{cases}
0,~&0\leq\text{acc}\leq 80;\\
\frac{\text{acc}-80}{13}\cdot\text{diff}',~&80<\text{acc}\leq93;\\
\frac{(\text{diffConst}-\text{diff}')(\text{acc}-93)}{3}+\text{diff}',~&93<\text{acc}\leq96;\\
\frac{3(\text{acc-96})}{6-(\text{acc-96})}+\text{diffConst},~&96<\text{acc}\leq98;\\
\frac{8(\text{acc-98})}{9-2(\text{acc-98})}+\text{diffConst}+1.5,~&98<\text{acc}\leq99.5;\\
\frac{4(\text{acc-99.5})}{3-2(\text{acc-99.5})}+\text{diffConst}+3.5,~&99.5<\text{acc}\leq100.
\end{cases}\end{equation}$$

简易速查：
|accuracy|rating|
|---|---|
|80|0|
|93| 定数-3|
|94| 定数-2|
|95| 定数-1|
|96| 定数|
|96.86| 定数+0.5|
|97.5| 定数+1|
|98| 定数+1.5|
|98.5| 定数+2|
|98.9| 定数+2.5|
|99.23| 定数+3|
|99.5| 定数+3.5|
|99.8| 定数+4|
|100| 定数+4.5|

对应Rust代码(基于原Excel公式，没有化简)

```rust
#[inline]
fn calc_rating(diff_const: f64, acc: f64) -> f64 {
    if acc < 0.0 || acc > 100.0 {
        return 0.0;
    }

    let diff_lower = (diff_const - 3.0).max(0.0);
    if acc <= 80.0 {
        0.0
    } else if acc <= 93.0 {
        diff_lower * (acc - 80.0) / 13.0
    } else if acc <= 96.0 {
        (diff_const - diff_lower) * (acc - 93.0) / 3.0 + diff_lower
    } else if acc <= 98.0 {
        let acc_xtra = acc - 96.0;
        1.5 * acc_xtra / (3.0 - acc_xtra / 2.0) + diff_const
    } else if acc <= 99.5 {
        let acc_xtra2 = (acc - 98.0) / 1.5;
        2.0 * acc_xtra2 * 2.0 / (3.0 - acc_xtra2) + diff_const + 1.5
    } else {
        let acc_xtra3 = (acc - 99.5) * 2.0;
        acc_xtra3 * 2.0 / (3.0 - acc_xtra3) + diff_const + 3.5
    }
}
```

## Acc计算
由于目前的版本对于LN的定级仍然较高，在Acc计算上进行手动进行干预。

$$\text{ratingAcc}=\frac{310\times\text{numMarvelous}+300\times\text{numPerfect}+200\times\text{numGreat}+100\times\text{numGood}+50\times\text{numBad}}{310\times(\text{numMarvelous}+\text{numPerfect}+\text{numGreat}+\text{numGood}+\text{numBad}+\text{numMiss})}\times100 \%$$

即osu!mania从高到低的6个判定依次给予310, 300, 200, 100, 50, 0的权重。相较之下，ScoreV2对彩色判定的权重是305，目前的Rank系统对彩色判定的权重是320。而经过计算后发现320权重会导致相同成绩开启ScoreV2模式的Rating更高，且目前Rating计算没有适应新的Acc；而305的权重会导致LN优势过大，因此目前采用310权重。

# TODO List
+ ~~内置指南~~ (0.1.1已加入)
+ 显示加载进度条 (可能不好刷新ui)
+ ~~导出后自动打开文件夹/图片？~~ (0.2.0已加入打开图片)
+ 移除卡片可以选择是否自动填充
+ 联网谱面验证、本地回放验证
+ 对HR, EZ等mod的支持
+ 可扩展的窗口大小
+ 更美观


# Update List
## v0.1.1
+ 内置了使用说明，便于理解操作，以及打开github主页的按钮
+ 暂时使用skia作为渲染器，生成的exe文件大了约18MB，使文字渲染效果更好（原生太烂了）
+ 加入了程序图标和窗口图标

## v0.2.0
+ 使用jpeg格式保存导出图像，显著减小图片大小
+ 导出图像后会自动使用`open`库打开
+ 为GUI中的卡片添加了复制按键，可以快速复制单个卡片到剪切板
+ 卡片中超过长度的标题和难度名会省略，只显示标题后面部分和难度前面部分
+ 添加了所有玩家的最佳成绩选项（"[All Players]"）
+ 修复了在新的卡片还没生成时点击导出按钮会导出旧卡片的问题，现在在加载过程中导出按钮会被禁用
+ 使用rosu-memory-lib库实现了测试中的实时导出功能：可以记录程序启动后游玩的成绩。
目前无法读取osu!启动后程序启动前的成绩，且在结算界面启动程序会无法读取使用的mod，因此暂时取消在osu中打开回放时生成成绩图的功能。
+ 目前存在的bug：ui更新不及时，有时候无法读取ResultScreen的内容，在osu关闭后实时读取线程会自动结束。