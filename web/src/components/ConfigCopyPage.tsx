import { useCharacterConfigs } from "../hooks/useTuning";
import ConfigCopyPanel from "./ConfigCopyPanel";

export default function ConfigCopyPage() {
  const { configs, loading, error } = useCharacterConfigs();

  if (loading) {
    return (
      <section className="p-6 font-tech text-white/70">
        Loading character configs...
      </section>
    );
  }

  if (error) {
    return (
      <section className="p-6 font-tech text-rose-400">
        Failed to load character configs: {error}
      </section>
    );
  }

  return <ConfigCopyPanel characterConfigs={configs} />;
}
