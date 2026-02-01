// Definition des blocs personnalises pour HTML
Blockly.Blocks['html_element'] = {
    init: function() {
        this.appendDummyInput()
            .appendField("HTML element")
            .appendField(new Blockly.FieldTextInput("div"), "TAG");
        this.appendStatementInput("CONTENT")
            .setCheck(null)
            .appendField("contenu");
        this.setColour(160);
        this.setTooltip('');
        this.setHelpUrl('');
    }
};

// Definition du generateur de code pour les blocs HTML
Blockly.JavaScript['html_element'] = function(block) {
    const tag = block.getFieldValue('TAG');
    const statements_content = Blockly.JavaScript.statementToCode(block, 'CONTENT');
    const code = `<${tag}>\n${statements_content}</${tag}>\n`;
    return code;
};

// Blocs personnalises pour CSS
Blockly.Blocks['css_style'] = {
    init: function() {
        this.appendDummyInput()
            .appendField("CSS style")
            .appendField(new Blockly.FieldTextInput("body { }"), "STYLE");
        this.setColour(120);
        this.setOutput(true, "String");
    }
};

// Definition du generateur de code pour les blocs CSS
Blockly.JavaScript['css_style'] = function(block) {
    const style = block.getFieldValue('STYLE');
    const code = `<style>\n${style}\n</style>\n`;
    return code;
};

// Blocs personnalises pour JavaScript
Blockly.Blocks['javascript_code'] = {
    init: function() {
        this.appendDummyInput()
            .appendField("JavaScript code")
            .appendField(new Blockly.FieldTextInput("console.log('Hello World');"), "CODE");
        this.setColour(210);
        this.setOutput(true, "String");
    }
};

// Definition du generateur de code pour les blocs JavaScript
Blockly.JavaScript['javascript_code'] = function(block) {
    const code = block.getFieldValue('CODE');
    return `<script>\n${code}\n</script>\n`;
};

// Configuration de Blockly
const workspace = Blockly.inject('blocklyDiv', {
    toolbox: `
        <xml xmlns="https://developers.google.com/blockly/xml">
            <block type="html_element"></block>
            <block type="css_style"></block>
            <block type="javascript_code"></block>
        </xml>
    `
});

// Fonction pour generer et executer le code
function runCode() {
    const code = Blockly.JavaScript.workspaceToCode(workspace);
    document.getElementById('code').value = code;

    // Executer le code en l'inserant dans le div de sortie
    const outputDiv = document.getElementById('output');
    outputDiv.innerHTML = code;
}
