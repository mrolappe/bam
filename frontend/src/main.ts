import { createApp } from "vue";
import "./style.css";
import App from "./App.vue";
import { bamClientKey } from "./composables/useBamClient";
import { HttpClient } from "./transport/HttpClient";

const app = createApp(App);
app.provide(bamClientKey, new HttpClient());
app.mount("#app");
