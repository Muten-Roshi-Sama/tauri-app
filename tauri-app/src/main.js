const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.core;


const testImgPath1 = "C:/Users/user/PPRO_extensions/Videos_temp/angry.jpeg"
const testImgPath2 = "C:/Users/user/PPRO_extensions/Videos_temp/angry2.jpeg"



//*______________UI Buttons & Info Showcase _______
let greetInputEl;
let greetMsgEl;
async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsgEl.textContent = await invoke("greet", { name: greetInputEl.value });
}
// Example project triggers
async function addMarker(timestamp) {
  await invoke("add_marker", { timestamp });
}







//_________Log Panel______________


//-------- Dev --------
function setupUI_dev(){
  const btn_start = document.getElementById("deepFaceTestButton_start");
  const btn_analyze = document.getElementById("deepFaceTestButton_analyze");
  const btn_verify = document.getElementById("deepFaceTestButton_verify");
  const btn_detect = document.getElementById("deepFaceTestButton_detect");


  btn_start.addEventListener("click", async () => {
    try {
      startDeepfaceServer()
    } catch (err) {
      console.error("DeepFace call failed:", err);
    }
  });

  btn_analyze.addEventListener("click", async () => {
    try {
      PerformDeepfaceTest("analyze")
    } catch (err) {
      console.error("DeepFace call failed:", err);
    }
  });

  btn_verify.addEventListener("click", async () => {
    try {
      PerformDeepfaceTest("verify")
    } catch (err) {
      console.error("DeepFace call failed:", err);
    }
  });

  btn_detect.addEventListener("click", async () => {
    try {
      PerformDeepfaceTest("detect")
    } catch (err) {
      console.error("DeepFace call failed:", err);
    }
  });

}

async function startDeepfaceServer(){
  await invoke("start_deepface_server", {port: 8765})
}
async function PerformDeepfaceTest(action){
  let res;
  switch (action){
    case "analyze":
      res = await invoke("analyze_deepface", {
        frame: testImgPath1,
        actions: "emotion",
        detector: "opencv"
        })
        break;
    case "verify":
        // verify two frames
      res = await invoke("verify_deepface", {
        img1: testImgPath1,
        img2: testImgPath2,
        detector: "opencv"
      })
      break;
    
    case "detect":
      res = await invoke("detect_deepface", {
        frame: testImgPath1,
        detector: "opencv"
      })

  }





}

// async function runDeepface_test() {
//   try {
//     const result = await invoke("analyze_deepface", {
//       frames: ["C:/Users/user/PPRO_extensions/Videos_temp/sampleLong/angry.jpeg"],
//       actions: "emotion",     // emotion NO:////age,gender,race
//       model: "VGG-Face",   // Optional : Facenet, Facenet512, OpenFace, DeepFace, DeepID, ArcFace, Dlib, SFace
//       detector: "opencv"   // (must be installed in .exe) : opencv, ssd, dlib, retinaface, mediapipe, yolov8
//     });
//     console.log("DeepFace result:", result);
//   } catch (err) {
//     console.error("DeepFace failed:", err);
//   }
// }





async function setupLicenseListener() {
  await listen("license-status", (event) => {
    const licenseEl = document.getElementById("license-status");
    licenseEl.textContent = event.payload;
    if (event.payload.includes("✅")) {
      licenseEl.style.color = "green";
    } else {
      licenseEl.style.color = "red";
    }
  });
}
function setupPipelinesStatus(){
   //*__________Pipelines Status Indicators________
  // CEP/WebSocket Status  
  window.__TAURI__.event.listen("cep-status", event => {
    console.log("Received cep-status event:", event.payload);
    const el = document.getElementById("status-tauri-cep");
    if (!el) {
      console.error("status-tauri-cep element not found!");
      return;
    }
    
    el.textContent = `🔌 CEP: ${event.payload}`;
    el.className = "status-indicator";
  });
  
  // Cloud Server Pipeline Status
  window.__TAURI__.event.listen("status-tauri-cloud", event => {
    console.log("Received status-tauri-cloud event:", event.payload);
    const el = document.getElementById("status-tauri-cloud");
    if (!el) {
      console.error("status-tauri-cloud element not found!");
      return;
    }
    
    el.textContent = `🌐 Cloud: ${event.payload}`;
    el.className = "status-indicator";
  });
}

//*_____________________________________________________

window.addEventListener("DOMContentLoaded", () => {
  greetInputEl = document.querySelector("#greet-input");
  greetMsgEl = document.querySelector("#greet-msg");
  document.querySelector("#greet-form").addEventListener("submit", (e) => {
    e.preventDefault();
    greet();
  });



  setupPipelinesStatus();

  setupUI_dev();

});
