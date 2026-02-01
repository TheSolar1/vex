<!DOCTYPE html>
<html lang="fr">
<head>
    <meta charset="UTF-8">
    <title>editeur de code visuel</title>
    <script src="https://unpkg.com/blockly/blockly.min.js"></script>
    <style>
        body {
            display: flex;
            flex-direction: column;
            align-items: center;
            font-family: Arial, sans-serif;
        }
        #blocklyDiv {
            height: 480px;
            width: 600px;
        }
        #code {
            width: 600px;
            height: 200px;
            border: 1px solid #ccc;
            margin-top: 20px;
        }
        #output {
            width: 600px;
            min-height: 200px;
            border: 1px solid #ccc;
            margin-top: 20px;
            padding: 10px;
        }
    </style>
</head>
<body>
    <h1>editeur de code visuel</h1>
    <div id="blocklyDiv"></div>
    <textarea id="code" readonly></textarea>
    <button onclick="runCode()">Executer le code</button>
    <div id="output"></div>
    <script src="main.js"></script>

    
</body>
</html>
