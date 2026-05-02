import { motion } from "framer-motion";

export function LogoAnimation(): JSX.Element {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12, scale: 0.95 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.4, ease: "easeOut" }}
      className="rounded-2xl border border-zinc-300/60 bg-white/70 px-5 py-3 text-zinc-900 shadow-lg backdrop-blur dark:border-zinc-700/80 dark:bg-zinc-900/70 dark:text-zinc-100"
    >
      Galcode Island
    </motion.div>
  );
}
