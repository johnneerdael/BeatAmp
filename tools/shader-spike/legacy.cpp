#include "HLSLParser.h"
#include "GLSLGenerator.h"
#include <fstream>
#include <iostream>
#include <sstream>
int main(int argc,char**argv) {
    if(argc!=3){std::cerr<<"usage: legacy-hlsl INPUT OUTPUT\n";return 2;}
    std::ifstream in(argv[1]);std::stringstream input;input<<in.rdbuf();auto source=input.str();
    if(!in||source.size()>1024*1024){std::cerr<<"invalid/oversized HLSL source\n";return 2;}
    M4::Allocator allocator;M4::HLSLTree tree(&allocator);M4::HLSLParser parser(&allocator,&tree);
    std::string preprocessed;
    if(!parser.ApplyPreprocessor(argv[1],source.c_str(),source.size(),preprocessed)){std::cerr<<"preprocessing failed\n";return 1;}
    if(!parser.Parse(argv[1],preprocessed.c_str(),preprocessed.size())){std::cerr<<"legacy HLSL parsing failed\n";return 1;}
    M4::GLSLGenerator generator;
    if(!generator.Generate(&tree,M4::GLSLGenerator::Target_FragmentShader,M4::GLSLGenerator::Version_330,"fragmentMain",M4::GLSLGenerator::Options(M4::GLSLGenerator::Flag_AlternateNanPropagation))){std::cerr<<"GLSL generation failed\n";return 1;}
    std::ofstream out(argv[2]);out<<generator.GetResult();return out?0:1;
}
