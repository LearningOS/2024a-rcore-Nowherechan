# lab3 report

## 简单总结你实现的功能（200字以内，不要贴代码）

实现了一个系统调用，叫做 `sys_task_info`，`syscall id` 是 `410`，该系统调用可以返回一个 TaskInfo 的结构体，内容包括：该进程当前状态（始终为 `Running`）、各个系统调用被调用的次数、系统调用时刻距离任务第一次被调度的时刻的时长，单位为 ms。

## 完成问答题
1. 正确进入 U 态后，程序的特征还应有：使用 S 态特权指令，访问 S 态寄存器后会报错。 请同学们可以自行测试这些内容（运行 三个 bad 测例 (ch2b_bad_*.rs) ）， 描述程序出错行为，同时注意注明你使用的 sbi 及其版本。

出错行为：
- [kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003a4, kernel killed it.
- [kernel] IllegalInstruction in application, kernel killed it.
- [kernel] IllegalInstruction in application, kernel killed it.

SBI 信息注明：`RustSBI version 0.4.0-alpha.1, adapting to RISC-V SBI v2.0.0`

2. 深入理解 trap.S 中两个函数 __alltraps 和 __restore 的作用，并回答如下问题:
    - L40：刚进入 __restore 时，a0 代表了什么值。请指出 __restore 的两种使用情景。
    - trap handler 的返回值，即 cx；1. 系统调用返回进入用户态；2. 内核启动后进入 init 用户进程。

    - L43-L48：这几行汇编代码特殊处理了哪些寄存器？这些寄存器的的值对于进入用户态有何意义？请分别解释。
    - 处理了 `sstatus, sepc, sscratch`，恢复用户态的状态、触发中断的指令、用户栈的地址

    - L50-L56：为何跳过了 x2 和 x4？
    - x2 是 sp，不急着恢复，最后再恢复，x4 用户态不用

    - L60：该指令之后，sp 和 sscratch 中的值分别有什么意义？
    - 换位了，sp 现在指向用户态栈，sscratch 指向内核栈

    - __restore：中发生状态切换在哪一条指令？为何该指令执行之后会进入用户态？
    - sret；isa 这样设计的

    - L13：该指令之后，sp 和 sscratch 中的值分别有什么意义？
    - 注释写着：now sp->kernel stack, sscratch->user stack

    - 从 U 态进入 S 态是哪一条指令发生的？
    - 是 `user/src/syscall.rs:52 ecall` 发生的


## 荣誉准则
在完成本次实验的过程（含此前学习的过程）中，我曾分别与以下各位就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

> Myself

此外，我也参考了以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

> riscv isa spec, xv6 book

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 你对本次实验设计及难度/工作量的看法，以及有哪些需要改进的地方，欢迎畅所欲言。

回答问题很累，每个阶段之间时间很短，可能没有那么多空闲时间。