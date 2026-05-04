// Facade module — 实际实现已经按职责拆到 4 个子模块：
//   - binary.rs   CLI binary 解析 + bundled runtime + version cache
//   - proc.rs     跨平台进程探查 + 杀进程 + 启动孤儿回收
//   - proxy.rs    HTTP/SOCKS5 代理 env + 终端 quote + 拉系统终端
//   - stream.rs   CLI 流事件发射 + JSON 工具 + 超时常量
//
// 保留 sysutils 作为统一 re-export 入口，下游可以 `use crate::agent::sysutils::*`
// 一次拿全。新代码鼓励直接 import 具体模块以避免名字污染。

pub use crate::agent::binary::*;
pub use crate::agent::proc::*;
pub use crate::agent::proxy::*;
pub use crate::agent::stream::*;
