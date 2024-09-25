// Fake Phone Call Functionality
function startFakeCall() {
    const fakeCallDiv = document.getElementById('fakeCall');
    fakeCallDiv.style.display = 'block';
    setTimeout(() => {
        alert("Incoming fake call...");
    }, 2000);  // Simulate ringing after 2 seconds
}

// End fake call function
function endCall() {
    const fakeCallDiv = document.getElementById('fakeCall');
    fakeCallDiv.style.display = 'none';
}

// Creep-O-Meter (Using TensorFlow.js for Sentiment Analysis)
function startCreepOMeter() {
    const creepOMeterDiv = document.getElementById('creepOMeter');
    creepOMeterDiv.style.display = 'block';

    setTimeout(() => {
        alert("Creep-O-Meter detected suspicious speech!");
    }, 5000);  // Simulate speech analysis
}

// Escape Route Planner (Using WebGL for AR Escape Routes)
function startEscapePlan() {
    const escapePlanDiv = document.getElementById('escapePlan');
    escapePlanDiv.style.display = 'block';

    setTimeout(() => {
        alert("Escape route detected! Follow the green line to the exit.");
    }, 3000);
