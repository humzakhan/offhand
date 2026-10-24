import { useState } from "react";
import "./App.css";

function App() {
  const [task, setTask] = useState("");

  return (
    <main className="container">
      <h1>Offhand</h1>
      <input
        aria-label="Task"
        value={task}
        onChange={(e) => setTask(e.currentTarget.value)}
        placeholder="What should I do?"
      />
      <p className="status">Idle</p>
    </main>
  );
}

export default App;
