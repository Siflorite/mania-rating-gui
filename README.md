# mania-rating-gui
A rating calcuator for osu!mania 6K mode.

# 使用方法：
打开程序后，程序会加载一段时间，包括读取本地osu的成绩，并绘制近30次成绩的卡片(Recent 30)。之后程序会显示这样的界面：

![主界面](/pics/main.jpg "主界面")

窗口的左上角的下拉框包含了所有本地osu!目录内有osu!mania 6K谱面游玩记录的玩家名称，以及Recent 30，通过选择玩家名称即可加载其最佳30个游玩记录(Best 30)并绘制卡片。

界面内的成绩卡片会在右下角有红色的"-""按钮或绿色的"+"按钮。在点击红色按钮之后，对应的卡片将会从成绩列表移除，添加到备选列表。如果玩家的成绩还有剩余，如在开始移除一个之后，游玩的记录数>31，那么就会在成绩列表自动填充剩余的最好的成绩。

而进入到备选列表的卡片右下角的按钮会变为绿色按钮，在点击按钮之后，会移除成绩列表最差的一个成绩，并将这个成绩卡片重新放回成绩列表。点击左上角的"重置"按钮即可重新将成绩列表设置为最佳的30个记录。

通过加减按钮处理了不想要的记录（如谱面未上传到官网，或者与游玩记录内有相同的谱面）后，即可点击导出按钮，导出成绩列表的所有卡片，导出位置将会在窗口右上角提示。

![导出](/pics/exported.jpg "导出")

导出的样例如下所示：

![导出图像](/pics/SiFouR.png "导出图像")

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

# 使用的算法

[sunnyxxy SR Reborn](https://github.com/sunnyxxy/Star-Rating-Rebirth)：使用2025/04/15版本。

Rating计算：使用sunnyxxy osu!主页展示的[google表格](https://docs.google.com/spreadsheets/d/1orVFRc_dmCDaQaIEGi1vcjePcZ0od0qMriuUNJOQUO0/edit?pli=1&gid=777965813#gid=777965813)中的公式：

$$\text{Params: } \text{diff}'=\max(\text{diff\_const}-3,0)$$

$$
\text{rating}=f(\text{acc}, \text{diff\_const})=
\begin{cases}
    0,~&0\leq\text{acc}\leq 80;\\
    \frac{\text{acc}-80}{13},~&80<\text{acc}\leq93;\\
    \frac{(\text{diff\_const}-\text{diff}')(\text{acc}-93)}{3}+\text{diff}',~&93<\text{acc}\leq96;\\
    \frac{3(\text{acc-96})}{6-(\text{acc-96})}+\text{diff\_const},~&96<\text{acc}\leq98;\\
    \frac{8(\text{acc-98})}{9-2(\text{acc-98})}+\text{diff\_const}+1.5,~&98<\text{acc}\leq99.5;\\
    \frac{4(\text{acc-99.5})}{3-2(\text{acc-99.5})}+\text{diff\_const}+3.5,~&99.5<\text{acc}\leq100.
\end{cases}
$$
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


# TODO List
+ 内置指南
+ 导出后自动打开文件夹/图片？
+ 移除卡片可以选择是否自动填充
+ 联网谱面验证、本地回放验证
+ 更美观